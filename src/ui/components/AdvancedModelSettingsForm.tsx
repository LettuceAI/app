import type { ChangeEvent } from "react";
import { Brain, Info } from "lucide-react";
import {
  normalizeLlamaSamplerOrder,
  type AdvancedModelSettings,
  type ReasoningSupport,
} from "../../core/storage/schemas";
import { cn } from "../design-tokens";
import { useI18n } from "../../core/i18n/context";
import { Switch } from "./Switch";

export const ADVANCED_TEMPERATURE_RANGE = { min: 0, max: 2 };
export const ADVANCED_TOP_P_RANGE = { min: 0, max: 1 };
export const ADVANCED_MAX_TOKENS_RANGE = { min: 0, max: 32768 };
export const ADVANCED_CONTEXT_LENGTH_RANGE = { min: 0, max: 262144 };
export const ADVANCED_FREQUENCY_PENALTY_RANGE = { min: -2, max: 2 };
export const ADVANCED_PRESENCE_PENALTY_RANGE = { min: -2, max: 2 };
export const ADVANCED_TOP_K_RANGE = { min: 1, max: 500 };
export const ADVANCED_SD_STEPS_RANGE = { min: 1, max: 150 };
export const ADVANCED_SD_CFG_SCALE_RANGE = { min: 1, max: 30 };
export const ADVANCED_SD_SEED_RANGE = { min: 0, max: 2_147_483_647 };
export const ADVANCED_SD_DENOISING_STRENGTH_RANGE = { min: 0, max: 1 };
export const ADVANCED_REASONING_BUDGET_RANGE = { min: 1024, max: 32768 };
export const ADVANCED_LLAMA_GPU_LAYERS_RANGE = { min: 0, max: 512 };
export const ADVANCED_LLAMA_THREADS_RANGE = { min: 1, max: 256 };
export const ADVANCED_LLAMA_THREADS_BATCH_RANGE = { min: 1, max: 256 };
export const ADVANCED_LLAMA_SEED_RANGE = { min: 0, max: 2_147_483_647 };
export const ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE = { min: 0, max: 1_000_000 };
export const ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE = { min: 0, max: 10 };
export const ADVANCED_LLAMA_BATCH_SIZE_RANGE = { min: 1, max: 8192 };
export const ADVANCED_OLLAMA_NUM_CTX_RANGE = { min: 0, max: 262_144 };
export const ADVANCED_OLLAMA_NUM_PREDICT_RANGE = { min: 0, max: 131_072 };
export const ADVANCED_OLLAMA_NUM_KEEP_RANGE = { min: 0, max: 32_768 };
export const ADVANCED_OLLAMA_NUM_BATCH_RANGE = { min: 1, max: 16_384 };
export const ADVANCED_OLLAMA_NUM_GPU_RANGE = { min: 0, max: 512 };
export const ADVANCED_OLLAMA_NUM_THREAD_RANGE = { min: 1, max: 256 };
export const ADVANCED_OLLAMA_TFS_Z_RANGE = { min: 0, max: 1 };
export const ADVANCED_OLLAMA_TYPICAL_P_RANGE = { min: 0, max: 1 };
export const ADVANCED_OLLAMA_MIN_P_RANGE = { min: 0, max: 1 };
export const ADVANCED_OLLAMA_MIROSTAT_RANGE = { min: 0, max: 2 };
export const ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE = { min: 0, max: 10 };
export const ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE = { min: 0, max: 1 };
export const ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE = { min: 0, max: 2 };
export const ADVANCED_OLLAMA_SEED_RANGE = { min: 0, max: 2_147_483_647 };

function clampValue(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export function sanitizeAdvancedModelSettings(input: AdvancedModelSettings): AdvancedModelSettings {
  const sanitize = (
    value: number | null | undefined,
    range: { min: number; max: number },
    toInteger = false,
  ) => {
    if (value === null || value === undefined) {
      return null;
    }
    const numeric = Number(value);
    if (!Number.isFinite(numeric)) {
      return null;
    }
    const clamped = clampValue(numeric, range.min, range.max);
    return toInteger ? Math.round(clamped) : Number(clamped.toFixed(3));
  };

  const normalizeStop = (value: unknown): string[] | null => {
    if (!Array.isArray(value)) return null;
    const cleaned = value
      .map((v) => (typeof v === "string" ? v.trim() : ""))
      .filter((v) => v.length > 0);
    return cleaned.length > 0 ? cleaned : null;
  };

  const normalizeSdSize = (value: unknown): string | null => {
    if (typeof value !== "string") return null;
    const trimmed = value.trim().toLowerCase().replace(/\s+/g, "");
    if (!trimmed) return null;
    return /^\d+x\d+$/.test(trimmed) ? trimmed : null;
  };

  return {
    temperature: sanitize(input.temperature, ADVANCED_TEMPERATURE_RANGE, false),
    topP: sanitize(input.topP, ADVANCED_TOP_P_RANGE, false),
    maxOutputTokens: sanitize(input.maxOutputTokens, ADVANCED_MAX_TOKENS_RANGE, true),
    contextLength: sanitize(input.contextLength, ADVANCED_CONTEXT_LENGTH_RANGE, true),
    frequencyPenalty: sanitize(input.frequencyPenalty, ADVANCED_FREQUENCY_PENALTY_RANGE, false),
    presencePenalty: sanitize(input.presencePenalty, ADVANCED_PRESENCE_PENALTY_RANGE, false),
    topK: sanitize(input.topK, ADVANCED_TOP_K_RANGE, true),
    sdSteps: sanitize(input.sdSteps, ADVANCED_SD_STEPS_RANGE, true),
    sdCfgScale: sanitize(input.sdCfgScale, ADVANCED_SD_CFG_SCALE_RANGE, false),
    sdSampler: input.sdSampler?.trim() || null,
    sdSeed: sanitize(input.sdSeed, ADVANCED_SD_SEED_RANGE, true),
    sdNegativePrompt: input.sdNegativePrompt?.trim() || null,
    sdDenoisingStrength: sanitize(
      input.sdDenoisingStrength,
      ADVANCED_SD_DENOISING_STRENGTH_RANGE,
      false,
    ),
    sdSize: normalizeSdSize(input.sdSize),
    llamaGpuLayers: sanitize(input.llamaGpuLayers, ADVANCED_LLAMA_GPU_LAYERS_RANGE, true),
    llamaThreads: sanitize(input.llamaThreads, ADVANCED_LLAMA_THREADS_RANGE, true),
    llamaThreadsBatch: sanitize(input.llamaThreadsBatch, ADVANCED_LLAMA_THREADS_BATCH_RANGE, true),
    llamaSeed: sanitize(input.llamaSeed, ADVANCED_LLAMA_SEED_RANGE, true),
    llamaRopeFreqBase: sanitize(
      input.llamaRopeFreqBase,
      ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE,
      false,
    ),
    llamaRopeFreqScale: sanitize(
      input.llamaRopeFreqScale,
      ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE,
      false,
    ),
    llamaOffloadKqv: input.llamaOffloadKqv ?? null,
    llamaBatchSize: sanitize(input.llamaBatchSize, ADVANCED_LLAMA_BATCH_SIZE_RANGE, true),
    llamaKvType: input.llamaKvType ?? null,
    llamaFlashAttention: input.llamaFlashAttention ?? null,
    llamaChatTemplateOverride: input.llamaChatTemplateOverride?.trim() || null,
    llamaMmprojPath: input.llamaMmprojPath?.trim() || null,
    llamaChatTemplatePreset: input.llamaChatTemplatePreset?.trim() || null,
    llamaRawCompletionFallback: input.llamaRawCompletionFallback ?? null,
    llamaStrictMode: input.llamaStrictMode ?? null,
    llamaSamplerProfile: input.llamaSamplerProfile ?? null,
    llamaSamplerOrder: normalizeLlamaSamplerOrder(input.llamaSamplerOrder),
    llamaMinP: sanitize(input.llamaMinP, ADVANCED_OLLAMA_MIN_P_RANGE, false),
    llamaTypicalP: sanitize(input.llamaTypicalP, ADVANCED_OLLAMA_TYPICAL_P_RANGE, false),
    llamaLastRuntimeReport: input.llamaLastRuntimeReport ?? null,
    ollamaNumCtx: sanitize(input.ollamaNumCtx, ADVANCED_OLLAMA_NUM_CTX_RANGE, true),
    ollamaNumPredict: sanitize(input.ollamaNumPredict, ADVANCED_OLLAMA_NUM_PREDICT_RANGE, true),
    ollamaNumKeep: sanitize(input.ollamaNumKeep, ADVANCED_OLLAMA_NUM_KEEP_RANGE, true),
    ollamaNumBatch: sanitize(input.ollamaNumBatch, ADVANCED_OLLAMA_NUM_BATCH_RANGE, true),
    ollamaNumGpu: sanitize(input.ollamaNumGpu, ADVANCED_OLLAMA_NUM_GPU_RANGE, true),
    ollamaNumThread: sanitize(input.ollamaNumThread, ADVANCED_OLLAMA_NUM_THREAD_RANGE, true),
    ollamaTfsZ: sanitize(input.ollamaTfsZ, ADVANCED_OLLAMA_TFS_Z_RANGE, false),
    ollamaTypicalP: sanitize(input.ollamaTypicalP, ADVANCED_OLLAMA_TYPICAL_P_RANGE, false),
    ollamaMinP: sanitize(input.ollamaMinP, ADVANCED_OLLAMA_MIN_P_RANGE, false),
    ollamaMirostat: sanitize(input.ollamaMirostat, ADVANCED_OLLAMA_MIROSTAT_RANGE, true),
    ollamaMirostatTau: sanitize(input.ollamaMirostatTau, ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE, false),
    ollamaMirostatEta: sanitize(input.ollamaMirostatEta, ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE, false),
    ollamaRepeatPenalty: sanitize(
      input.ollamaRepeatPenalty,
      ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE,
      false,
    ),
    ollamaSeed: sanitize(input.ollamaSeed, ADVANCED_OLLAMA_SEED_RANGE, true),
    ollamaStop: normalizeStop(input.ollamaStop),
    reasoningEnabled: input.reasoningEnabled ?? null,
    reasoningEffort: input.reasoningEffort ?? null,
    reasoningBudgetTokens: sanitize(
      input.reasoningBudgetTokens,
      ADVANCED_REASONING_BUDGET_RANGE,
      true,
    ),
    promptCachingEnabled: input.promptCachingEnabled ?? null,
    promptCachingTtl: input.promptCachingTtl ?? "5min",
  };
}

export function formatAdvancedModelSettingsSummary(
  settings: AdvancedModelSettings | null | undefined,
  fallbackLabel: string,
): string {
  if (!settings) {
    return fallbackLabel;
  }

  const formatValue = (value: number | null | undefined, digits = 2): string | null => {
    if (
      value === null ||
      value === undefined ||
      typeof value !== "number" ||
      !Number.isFinite(value)
    ) {
      return null;
    }
    return value % 1 === 0 ? value.toString() : Number(value).toFixed(digits);
  };

  const parts: string[] = [];
  const temperatureValue = formatValue(settings.temperature);
  if (temperatureValue) {
    parts.push(`Temp ${temperatureValue}`);
  }

  const topPValue = formatValue(settings.topP);
  if (topPValue) {
    parts.push(`Top P ${topPValue}`);
  }

  const maxTokensValue = formatValue(settings.maxOutputTokens, 0);
  if (maxTokensValue) {
    parts.push(`Max ${maxTokensValue}`);
  }

  const contextValue = formatValue(settings.contextLength, 0);
  if (contextValue) {
    parts.push(`Ctx ${contextValue}`);
  }

  const freqPenaltyValue = formatValue(settings.frequencyPenalty);
  if (freqPenaltyValue) {
    parts.push(`Freq ${freqPenaltyValue}`);
  }

  const presPenaltyValue = formatValue(settings.presencePenalty);
  if (presPenaltyValue) {
    parts.push(`Pres ${presPenaltyValue}`);
  }

  const topKValue = formatValue(settings.topK, 0);
  if (topKValue) {
    parts.push(`Top-K ${topKValue}`);
  }

  // Reasoning settings
  if (settings.reasoningEnabled === false) {
    parts.push("Reasoning: Off");
  } else if (settings.reasoningEnabled) {
    if (settings.reasoningEffort) {
      parts.push(`Reasoning: ${settings.reasoningEffort}`);
    }
    const budgetValue = formatValue(settings.reasoningBudgetTokens, 0);
    if (budgetValue) {
      parts.push(`Budget: ${budgetValue}`);
    }
  }

  // Prompt caching
  if (settings.promptCachingEnabled) {
    parts.push("Caching: On");
  }

  return parts.length ? parts.join(" • ") : fallbackLabel;
}

interface AdvancedModelSettingsFormProps {
  settings: AdvancedModelSettings;
  onChange: (settings: AdvancedModelSettings) => void;
  disabled?: boolean;
  /** The reasoning support type for the current provider */
  reasoningSupport?: ReasoningSupport;
}

export function AdvancedModelSettingsForm({
  settings,
  onChange,
  disabled,
  reasoningSupport = "none",
}: AdvancedModelSettingsFormProps) {
  const { t } = useI18n();
  const handleNumberChange =
    (key: keyof AdvancedModelSettings) => (event: ChangeEvent<HTMLInputElement>) => {
      const raw = event.target.value;
      const nextValue = raw === "" ? null : Number(raw);
      onChange({
        ...settings,
        [key]: nextValue,
      });
    };
  const inputClassName =
    "w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2.5 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50";

  // Check if we should show effort options
  const showEffortOptions = reasoningSupport === "effort" || reasoningSupport === "dynamic";
  const showReasoningSection = reasoningSupport !== "none";
  const isAutoReasoning = reasoningSupport === "auto";

  return (
    <div className="space-y-4">
      {/* Temperature */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.temperature")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.temperatureDesc")}
            </p>
          </div>
          <span className="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-xs font-mono text-white/90">
            {settings.temperature?.toFixed(2) ?? "0.70"}
          </span>
        </div>
        <input
          type="number"
          inputMode="decimal"
          min={ADVANCED_TEMPERATURE_RANGE.min}
          max={ADVANCED_TEMPERATURE_RANGE.max}
          step={0.01}
          value={settings.temperature ?? ""}
          onChange={handleNumberChange("temperature")}
          disabled={disabled}
          placeholder="0.70"
          className={inputClassName}
        />
        <div className="mt-1.5 flex justify-between text-[10px] text-white/40">
          <span>{ADVANCED_TEMPERATURE_RANGE.min}</span>
          <span>{ADVANCED_TEMPERATURE_RANGE.max}</span>
        </div>
      </div>

      {/* Top P */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.topP")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.topPDesc")}
            </p>
          </div>
          <span className="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-xs font-mono text-white/90">
            {settings.topP?.toFixed(2) ?? "1.00"}
          </span>
        </div>
        <input
          type="number"
          inputMode="decimal"
          min={ADVANCED_TOP_P_RANGE.min}
          max={ADVANCED_TOP_P_RANGE.max}
          step={0.01}
          value={settings.topP ?? ""}
          onChange={handleNumberChange("topP")}
          disabled={disabled}
          placeholder="1.00"
          className={inputClassName}
        />
        <div className="mt-1.5 flex justify-between text-[10px] text-white/40">
          <span>{ADVANCED_TOP_P_RANGE.min}</span>
          <span>{ADVANCED_TOP_P_RANGE.max}</span>
        </div>
      </div>

      {/* Max Output Tokens */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.maxTokens")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.maxTokensDesc")}
            </p>
          </div>
        </div>
        <input
          type="number"
          min={ADVANCED_MAX_TOKENS_RANGE.min}
          max={ADVANCED_MAX_TOKENS_RANGE.max}
          value={settings.maxOutputTokens ?? ""}
          onChange={handleNumberChange("maxOutputTokens")}
          disabled={disabled}
          placeholder="1024"
          className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2.5 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
        />
      </div>

      {/* Context Length */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.contextLength")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.contextLengthDesc")}
            </p>
          </div>
          <span className="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-xs font-mono text-white/90">
            {settings.contextLength
              ? settings.contextLength
              : t("components.advancedModelSettings.contextLengthAuto")}
          </span>
        </div>
        <input
          type="number"
          min={ADVANCED_CONTEXT_LENGTH_RANGE.min}
          max={ADVANCED_CONTEXT_LENGTH_RANGE.max}
          value={settings.contextLength ?? ""}
          onChange={(event) => {
            const raw = event.target.value;
            const nextValue = raw === "" ? null : Number(raw);
            onChange({
              ...settings,
              contextLength:
                nextValue === null || !Number.isFinite(nextValue) || nextValue === 0
                  ? null
                  : Math.trunc(nextValue),
            });
          }}
          disabled={disabled}
          placeholder={t("components.advancedModelSettings.contextLengthAuto")}
          className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2.5 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
        />
        <div className="mt-1.5 flex justify-between text-[10px] text-white/40">
          <span>{t("components.advancedModelSettings.contextLengthAuto")}</span>
          <span>{ADVANCED_CONTEXT_LENGTH_RANGE.max.toLocaleString()}</span>
        </div>
      </div>

      {/* Frequency Penalty */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.frequencyPenalty")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.frequencyPenaltyDesc")}
            </p>
          </div>
          <span className="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-xs font-mono text-white/90">
            {settings.frequencyPenalty?.toFixed(2) ?? "0.00"}
          </span>
        </div>
        <input
          type="number"
          inputMode="decimal"
          min={ADVANCED_FREQUENCY_PENALTY_RANGE.min}
          max={ADVANCED_FREQUENCY_PENALTY_RANGE.max}
          step={0.01}
          value={settings.frequencyPenalty ?? ""}
          onChange={handleNumberChange("frequencyPenalty")}
          disabled={disabled}
          placeholder="0.00"
          className={inputClassName}
        />
        <div className="mt-1.5 flex justify-between text-[10px] text-white/40">
          <span>{ADVANCED_FREQUENCY_PENALTY_RANGE.min}</span>
          <span>{ADVANCED_FREQUENCY_PENALTY_RANGE.max}</span>
        </div>
      </div>

      {/* Presence Penalty */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.presencePenalty")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.presencePenaltyDesc")}
            </p>
          </div>
          <span className="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-xs font-mono text-white/90">
            {settings.presencePenalty?.toFixed(2) ?? "0.00"}
          </span>
        </div>
        <input
          type="number"
          inputMode="decimal"
          min={ADVANCED_PRESENCE_PENALTY_RANGE.min}
          max={ADVANCED_PRESENCE_PENALTY_RANGE.max}
          step={0.01}
          value={settings.presencePenalty ?? ""}
          onChange={handleNumberChange("presencePenalty")}
          disabled={disabled}
          placeholder="0.00"
          className={inputClassName}
        />
        <div className="mt-1.5 flex justify-between text-[10px] text-white/40">
          <span>{ADVANCED_PRESENCE_PENALTY_RANGE.min}</span>
          <span>{ADVANCED_PRESENCE_PENALTY_RANGE.max}</span>
        </div>
      </div>

      {/* Top K */}
      <div className="rounded-xl border border-white/10 bg-white/5 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
              {t("components.advancedModelSettings.topK")}
            </label>
            <p className="mt-0.5 text-[11px] text-white/50">
              {t("components.advancedModelSettings.topKDesc")}
            </p>
          </div>
        </div>
        <input
          type="number"
          min={ADVANCED_TOP_K_RANGE.min}
          max={ADVANCED_TOP_K_RANGE.max}
          value={settings.topK ?? ""}
          onChange={handleNumberChange("topK")}
          disabled={disabled}
          placeholder="40"
          className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2.5 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
        />
      </div>

      {/* Prompt Caching Section */}
      <div className="space-y-4 rounded-2xl border border-blue-400/20 bg-blue-400/5 p-4 mb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-white">
            <h3 className="text-sm font-semibold">{t("components.extra.promptCachingTitle")}</h3>
          </div>
          <Switch
            checked={!!settings.promptCachingEnabled}
            onChange={(next) => onChange({ ...settings, promptCachingEnabled: next })}
            disabled={disabled}
          />
        </div>
        <p className="text-[11px] text-white/50 leading-relaxed">
          {t("components.extra.promptCachingDescription")}
        </p>
      </div>

      {/* Reasoning / Thinking Section */}
      {showReasoningSection && (
        <div className="space-y-4 rounded-2xl border border-amber-400/20 bg-amber-400/5 p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-white">
              <Brain className="h-4 w-4 text-amber-400" />
              <h3 className="text-sm font-semibold">
                {t("components.advancedModelSettings.reasoning")}
              </h3>
            </div>

            {!isAutoReasoning && (
              <Switch
                checked={!!settings.reasoningEnabled}
                onChange={(next) => onChange({ ...settings, reasoningEnabled: next })}
                disabled={disabled}
              />
            )}
          </div>

          <p className="text-[11px] text-white/50 leading-relaxed">
            {isAutoReasoning
              ? t("components.advancedModelSettings.reasoningAutoDesc")
              : t("components.advancedModelSettings.reasoningEnableDesc")}
          </p>

          {(settings.reasoningEnabled || isAutoReasoning) && (
            <div className="space-y-4 pt-2">
              {/* Mode Toggle for Dynamic Support (OpenRouter) - exclusive choice */}
              {reasoningSupport === "dynamic" &&
                (() => {
                  // Explicit mode detection: budget mode is active when budget has a truthy value AND effort is null/undefined
                  const isBudgetMode =
                    Boolean(settings.reasoningBudgetTokens) && !settings.reasoningEffort;

                  return (
                    <>
                      <div className="flex gap-2 p-1 rounded-xl bg-black/30 border border-white/5">
                        <button
                          type="button"
                          onClick={() => onChange({ ...settings, reasoningBudgetTokens: null })}
                          className={cn(
                            "flex-1 py-1.5 text-[10px] font-bold uppercase tracking-wider rounded-lg transition-all",
                            !isBudgetMode
                              ? "bg-amber-500/20 text-amber-200 border border-amber-500/30"
                              : "text-white/40 hover:text-white/60",
                          )}
                        >
                          {t("components.advancedModelSettings.effortMode")}
                        </button>
                        <button
                          type="button"
                          onClick={() =>
                            onChange({
                              ...settings,
                              reasoningEffort: null,
                              reasoningBudgetTokens: 8192,
                            })
                          }
                          className={cn(
                            "flex-1 py-1.5 text-[10px] font-bold uppercase tracking-wider rounded-lg transition-all",
                            isBudgetMode
                              ? "bg-amber-500/20 text-amber-200 border border-amber-500/30"
                              : "text-white/40 hover:text-white/60",
                          )}
                        >
                          {t("components.advancedModelSettings.budgetMode")}
                        </button>
                      </div>

                      {/* Effort controls - shown when NOT in budget mode */}
                      {!isBudgetMode && (
                        <div className="rounded-xl border border-amber-400/30 bg-black/20 p-4">
                          <div className="mb-3">
                            <label className="text-xs font-medium uppercase tracking-wider text-amber-200/80">
                              {t("components.advancedModelSettings.reasoningEffort")}
                            </label>
                            <p className="mt-0.5 text-[11px] text-white/50">
                              {t("components.advancedModelSettings.reasoningEffortDesc")}
                            </p>
                          </div>

                          <div className="grid grid-cols-4 gap-2">
                            {[
                              {
                                value: null,
                                label: t("components.advancedModelSettings.reasoningEffortAuto"),
                              },
                              {
                                value: "low" as const,
                                label: t("components.advancedModelSettings.reasoningEffortLow"),
                              },
                              {
                                value: "medium" as const,
                                label: t("components.advancedModelSettings.reasoningEffortMed"),
                              },
                              {
                                value: "high" as const,
                                label: t("components.advancedModelSettings.reasoningEffortHigh"),
                              },
                            ].map(({ value, label }) => (
                              <button
                                key={label}
                                type="button"
                                onClick={() => onChange({ ...settings, reasoningEffort: value })}
                                disabled={disabled}
                                className={cn(
                                  "rounded-lg border px-2 py-2 text-xs font-medium transition-all active:scale-[0.98] disabled:opacity-50",
                                  settings.reasoningEffort === value
                                    ? "border-amber-400/40 bg-amber-400/20 text-amber-100"
                                    : "border-white/10 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white",
                                )}
                              >
                                {label}
                              </button>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Budget controls - shown when IN budget mode */}
                      {isBudgetMode && (
                        <div className="rounded-xl border border-white/10 bg-black/20 p-4">
                          <div className="mb-3">
                            <label className="text-xs font-medium uppercase tracking-wider text-white/70">
                              {t("components.advancedModelSettings.reasoningBudget")}
                            </label>
                            <p className="mt-0.5 text-[11px] text-white/50">
                              {t("components.advancedModelSettings.reasoningBudgetDesc")}
                            </p>
                          </div>
                          <input
                            type="number"
                            min={ADVANCED_REASONING_BUDGET_RANGE.min}
                            max={ADVANCED_REASONING_BUDGET_RANGE.max}
                            value={settings.reasoningBudgetTokens ?? ""}
                            onChange={handleNumberChange("reasoningBudgetTokens")}
                            disabled={disabled}
                            placeholder="8192"
                            className="w-full rounded-xl border border-white/10 bg-black/30 px-3 py-2.5 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
                          />
                        </div>
                      )}
                    </>
                  );
                })()}

              {/* Non-dynamic providers: show effort options if supported */}
              {reasoningSupport !== "dynamic" && showEffortOptions && (
                <div className="rounded-xl border border-amber-400/30 bg-black/20 p-4">
                  <div className="mb-3">
                    <label className="text-xs font-medium uppercase tracking-wider text-amber-200/80">
                      {t("components.advancedModelSettings.reasoningEffort")}
                    </label>
                    <p className="mt-0.5 text-[11px] text-white/50">
                      {t("components.advancedModelSettings.reasoningEffortDesc")}
                    </p>
                  </div>

                  <div className="grid grid-cols-4 gap-2">
                    <button
                      type="button"
                      onClick={() => onChange({ ...settings, reasoningEffort: null })}
                      disabled={disabled}
                      className={cn(
                        "rounded-lg border px-3 py-2 text-xs font-medium transition-all active:scale-[0.98] disabled:opacity-50",
                        settings.reasoningEffort === null
                          ? "border-amber-400/40 bg-amber-400/20 text-amber-100"
                          : "border-white/10 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white",
                      )}
                    >
                      {t("components.advancedModelSettings.reasoningEffortAuto")}
                    </button>
                    {(["low", "medium", "high"] as const).map((effort) => {
                      const effortLabels = {
                        low: t("components.advancedModelSettings.reasoningEffortLow"),
                        medium: t("components.advancedModelSettings.reasoningEffortMed"),
                        high: t("components.advancedModelSettings.reasoningEffortHigh"),
                      } as const;
                      return (
                        <button
                          key={effort}
                          type="button"
                          onClick={() => onChange({ ...settings, reasoningEffort: effort })}
                          disabled={disabled}
                          className={cn(
                            "rounded-lg border px-3 py-2 text-xs font-medium transition-all active:scale-[0.98] disabled:opacity-50",
                            settings.reasoningEffort === effort
                              ? "border-amber-400/40 bg-amber-400/20 text-amber-100"
                              : "border-white/10 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white",
                          )}
                        >
                          {effortLabels[effort]}
                        </button>
                      );
                    })}
                  </div>

                  {settings.reasoningEffort && (
                    <div className="mt-3 flex items-start gap-2 rounded-lg bg-black/30 p-2">
                      <Info className="h-3 w-3 shrink-0 text-amber-400/60 mt-0.5" />
                      <p className="text-[10px] text-white/40">
                        {settings.reasoningEffort === "low" &&
                          t("components.advancedModelSettings.reasoningEffortLowDesc")}
                        {settings.reasoningEffort === "medium" &&
                          t("components.advancedModelSettings.reasoningEffortMediumDesc")}
                        {settings.reasoningEffort === "high" &&
                          t("components.advancedModelSettings.reasoningEffortHighDesc")}
                      </p>
                    </div>
                  )}
                </div>
              )}

              {/* Non-dynamic providers: show budget if budget-only */}
              {reasoningSupport === "budget-only" && (
                <div className="rounded-xl border border-white/10 bg-black/20 p-4">
                  <div className="mb-3">
                    <label className="text-xs font-medium uppercase tracking-wider text-white/70">
                      {t("components.advancedModelSettings.reasoningBudget")}
                    </label>
                    <p className="mt-0.5 text-[11px] text-white/50">
                      {t("components.advancedModelSettings.reasoningBudgetExtendedDesc")}
                    </p>
                  </div>
                  <input
                    type="number"
                    min={ADVANCED_REASONING_BUDGET_RANGE.min}
                    max={ADVANCED_REASONING_BUDGET_RANGE.max}
                    value={settings.reasoningBudgetTokens ?? ""}
                    onChange={handleNumberChange("reasoningBudgetTokens")}
                    disabled={disabled}
                    placeholder="8192"
                    className="w-full rounded-xl border border-white/10 bg-black/30 px-3 py-2.5 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
                  />
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
