import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ArrowLeft, RotateCcw, Save, Info, Loader } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { WindowControlButtons, useDragRegionProps } from "../../../components/App/TopNav";
import { cn } from "../../../design-tokens";
import type { AdvancedModelSettings } from "../../../../core/storage/schemas";
import { LlamaSamplerOrderEditor } from "../../../components/LlamaSamplerOrderEditor";
import { Switch } from "../../../components/Switch";
import {
  ADVANCED_TEMPERATURE_RANGE,
  ADVANCED_TOP_P_RANGE,
  ADVANCED_MAX_TOKENS_RANGE,
  ADVANCED_FREQUENCY_PENALTY_RANGE,
  ADVANCED_PRESENCE_PENALTY_RANGE,
  ADVANCED_TOP_K_RANGE,
  ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE,
  ADVANCED_LLAMA_DRY_BASE_RANGE,
  ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE,
  ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE,
} from "../../../components/AdvancedModelSettingsForm";

interface ParameterFieldProps {
  label: string;
  description: string;
  value: number | null | undefined;
  placeholder: string;
  min: number;
  max: number;
  step: number;
  onChange: (value: number | null) => void;
  inputMode?: "decimal" | "numeric";
  rangeLabels?: [string, string];
  disabled?: boolean;
}

function ParameterField({
  label,
  description,
  value,
  placeholder,
  min,
  max,
  step,
  onChange,
  inputMode = "decimal",
  rangeLabels,
  disabled = false,
}: ParameterFieldProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-baseline justify-between gap-2">
        <label className="text-[13px] font-medium text-fg/80">{label}</label>
        <span className="font-mono text-[12px] text-accent/80">
          {value != null ? (step < 1 ? value.toFixed(2) : String(value)) : placeholder}
        </span>
      </div>
      <p className="text-[11px] text-fg/40 leading-relaxed">{description}</p>
      <input
        type="number"
        inputMode={inputMode}
        min={min}
        max={max}
        step={step}
        value={value ?? ""}
        onChange={(e) => {
          const raw = e.target.value;
          onChange(raw === "" ? null : Number(raw));
        }}
        disabled={disabled}
        placeholder={placeholder}
        className={cn(
          "w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg placeholder-fg/30 transition focus:border-fg/20 focus:outline-none",
          disabled && "cursor-not-allowed opacity-60",
        )}
      />
      {rangeLabels && (
        <div className="flex justify-between text-[10px] text-fg/30">
          <span>{rangeLabels[0]}</span>
          <span>{rangeLabels[1]}</span>
        </div>
      )}
    </div>
  );
}

interface SessionAdvancedSettingsProps {
  isOpen: boolean;
  onClose: () => void;
  draft: AdvancedModelSettings;
  onDraftChange: (draft: AdvancedModelSettings) => void;
  overrideEnabled: boolean;
  onOverrideEnabledChange: (enabled: boolean) => void;
  baseSettings: AdvancedModelSettings;
  onSave: (settings: AdvancedModelSettings | null) => void;
  onShowParameterSupport: () => void;
  hasSession: boolean;
  providerId?: string;
  modelPath?: string;
}

export function SessionAdvancedSettings({
  isOpen,
  onClose,
  draft,
  onDraftChange,
  overrideEnabled,
  onOverrideEnabledChange,
  baseSettings,
  onSave,
  onShowParameterSupport,
  hasSession,
  providerId = "openai",
  modelPath,
}: SessionAdvancedSettingsProps) {
  const isLlama = providerId === "llamacpp";
  const dragRegionProps = useDragRegionProps();

  // Context info for llama models
  const [contextInfo, setContextInfo] = useState<{
    maxContextLength: number;
    recommendedContextLength?: number | null;
    availableMemoryBytes?: number | null;
    availableVramBytes?: number | null;
    modelSizeBytes?: number | null;
    supportsGpuOffload?: boolean | null;
  } | null>(null);
  const [contextLoading, setContextLoading] = useState(false);
  const isCpuOnlyLlamaBackend = isLlama && contextInfo?.supportsGpuOffload === false;

  useEffect(() => {
    if (!isOpen || !isLlama || !modelPath) return;
    let cancelled = false;
    setContextLoading(true);
    invoke<any>("llamacpp_context_info", {
      modelPath,
      llamaOffloadKqv: draft.llamaOffloadKqv ?? null,
      llamaKvType: draft.llamaKvType ?? null,
      llamaGpuLayers: draft.llamaGpuLayers ?? null,
    })
      .then((info) => {
        if (!cancelled) setContextInfo(info);
      })
      .catch(() => {
        if (!cancelled) setContextInfo(null);
      })
      .finally(() => {
        if (!cancelled) setContextLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen, isLlama, modelPath, draft.llamaOffloadKqv, draft.llamaKvType, draft.llamaGpuLayers]);

  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, onClose]);

  useEffect(() => {
    if (!isCpuOnlyLlamaBackend) {
      return;
    }
    if (draft.llamaGpuLayers === 0 && draft.llamaOffloadKqv === false) {
      return;
    }
    onDraftChange({
      ...draft,
      llamaGpuLayers: 0,
      llamaOffloadKqv: false,
    });
  }, [draft, isCpuOnlyLlamaBackend, onDraftChange]);

  const handleReset = () => {
    onOverrideEnabledChange(false);
    onDraftChange(baseSettings);
    onSave(null);
  };

  const handleSave = () => {
    onSave(overrideEnabled ? draft : null);
  };

  const update = (patch: Partial<AdvancedModelSettings>) => {
    onDraftChange({ ...draft, ...patch });
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="fixed inset-0 z-50 flex h-full flex-col bg-surface"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 20 }}
          transition={{ duration: 0.2 }}
        >
          {/* Header */}
          <div
            className="flex shrink-0 items-center justify-between border-b border-fg/10 px-4 py-3"
            {...dragRegionProps}
          >
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={onClose}
                className="rounded-full border border-fg/10 p-2 text-fg/60 transition hover:bg-fg/10 hover:text-fg"
                aria-label="Go back"
              >
                <ArrowLeft size={16} />
              </button>
              <div>
                <h2 className="text-base font-semibold text-fg">Session Parameters</h2>
                <p className="text-xs text-fg/45">Override model defaults for this conversation</p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onShowParameterSupport}
                className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/60 transition hover:bg-fg/10 hover:text-fg"
              >
                <Info size={14} className="inline mr-1" />
                Support
              </button>
              <button
                type="button"
                onClick={handleReset}
                className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/60 transition hover:bg-fg/10 hover:text-fg"
              >
                <RotateCcw size={14} className="inline mr-1" />
                Reset
              </button>
              <button
                type="button"
                onClick={handleSave}
                className={cn(
                  "rounded-full px-3 py-1.5 text-xs font-semibold text-fg transition",
                  "bg-linear-to-r from-accent to-accent/80",
                  "hover:from-accent/80 hover:to-accent/60",
                )}
              >
                <Save size={14} className="inline mr-1" />
                Save
              </button>
              <WindowControlButtons />
            </div>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto px-4 py-6">
            <div className="mx-auto max-w-3xl space-y-6">
              {!hasSession && (
                <div className="rounded-xl border border-warning/20 bg-warning/10 px-4 py-3 text-sm text-warning/80">
                  Open a chat session to configure per-session settings.
                </div>
              )}

              {hasSession && (
                <>
                  {/* Override toggle */}
                  <div className="flex items-center justify-between rounded-xl border border-fg/10 bg-fg/[0.03] px-4 py-3">
                    <div>
                      <p className="text-sm font-medium text-fg/90">Override defaults</p>
                      <p className="mt-0.5 text-xs text-fg/45">
                        Customize parameters just for this conversation
                      </p>
                    </div>
                    <Switch checked={overrideEnabled} onChange={onOverrideEnabledChange} />
                  </div>

                  {overrideEnabled && (
                    <>
                      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                        {/* Sampling */}
                        <div className="space-y-5 rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
                          <h3 className="text-[11px] font-bold uppercase tracking-wider text-fg/40">
                            Sampling
                          </h3>

                          <ParameterField
                            label="Temperature"
                            description="Controls randomness. Lower = more deterministic, higher = more creative."
                            value={draft.temperature}
                            placeholder="0.70"
                            min={ADVANCED_TEMPERATURE_RANGE.min}
                            max={ADVANCED_TEMPERATURE_RANGE.max}
                            step={0.01}
                            onChange={(v) => update({ temperature: v })}
                            rangeLabels={["Precise", "Creative"]}
                          />

                          <ParameterField
                            label="Top P"
                            description="Nucleus sampling. Limits tokens to a cumulative probability."
                            value={draft.topP}
                            placeholder="1.00"
                            min={ADVANCED_TOP_P_RANGE.min}
                            max={ADVANCED_TOP_P_RANGE.max}
                            step={0.01}
                            onChange={(v) => update({ topP: v })}
                            rangeLabels={["Focused", "Diverse"]}
                          />

                          <ParameterField
                            label="Top K"
                            description="Limits sampling to the top K most likely tokens."
                            value={draft.topK}
                            placeholder="40"
                            min={ADVANCED_TOP_K_RANGE.min}
                            max={ADVANCED_TOP_K_RANGE.max}
                            step={1}
                            onChange={(v) => update({ topK: v })}
                            inputMode="numeric"
                          />
                        </div>

                        {/* Output & Penalties */}
                        <div className="space-y-5 rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
                          <h3 className="text-[11px] font-bold uppercase tracking-wider text-fg/40">
                            Output & Penalties
                          </h3>

                          <div className="space-y-2">
                            <div className="flex items-baseline justify-between gap-2">
                              <label className="text-[13px] font-medium text-fg/80">
                                Max Output Tokens
                              </label>
                            </div>
                            <p className="text-[11px] text-fg/40 leading-relaxed">
                              Maximum response length. Auto lets the model decide.
                            </p>
                            <div className="flex gap-2">
                              <button
                                type="button"
                                onClick={() => update({ maxOutputTokens: null })}
                                className={cn(
                                  "flex-1 rounded-lg px-3 py-2 text-xs font-medium transition",
                                  !draft.maxOutputTokens
                                    ? "bg-accent/15 text-accent"
                                    : "border border-fg/10 text-fg/50 hover:bg-fg/5",
                                )}
                              >
                                Auto
                              </button>
                              <button
                                type="button"
                                onClick={() => update({ maxOutputTokens: 2048 })}
                                className={cn(
                                  "flex-1 rounded-lg px-3 py-2 text-xs font-medium transition",
                                  draft.maxOutputTokens
                                    ? "bg-accent/15 text-accent"
                                    : "border border-fg/10 text-fg/50 hover:bg-fg/5",
                                )}
                              >
                                Custom
                              </button>
                            </div>
                            {draft.maxOutputTokens != null && (
                              <input
                                type="number"
                                inputMode="numeric"
                                min={ADVANCED_MAX_TOKENS_RANGE.min}
                                max={ADVANCED_MAX_TOKENS_RANGE.max}
                                value={draft.maxOutputTokens ?? ""}
                                onChange={(e) =>
                                  update({ maxOutputTokens: Number(e.target.value) })
                                }
                                placeholder="2048"
                                className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg placeholder-fg/30 transition focus:border-fg/20 focus:outline-none"
                              />
                            )}
                          </div>

                          <ParameterField
                            label="Frequency Penalty"
                            description="Reduces repetition of token sequences."
                            value={draft.frequencyPenalty}
                            placeholder="0.00"
                            min={ADVANCED_FREQUENCY_PENALTY_RANGE.min}
                            max={ADVANCED_FREQUENCY_PENALTY_RANGE.max}
                            step={0.01}
                            onChange={(v) => update({ frequencyPenalty: v })}
                            rangeLabels={["Repeat", "Vary"]}
                          />

                          <ParameterField
                            label="Presence Penalty"
                            description="Encourages exploring new topics."
                            value={draft.presencePenalty}
                            placeholder="0.00"
                            min={ADVANCED_PRESENCE_PENALTY_RANGE.min}
                            max={ADVANCED_PRESENCE_PENALTY_RANGE.max}
                            step={0.01}
                            onChange={(v) => update({ presencePenalty: v })}
                            rangeLabels={["Repeat", "Explore"]}
                          />
                        </div>
                      </div>

                      {/* Runtime (llama.cpp only) */}
                      {isLlama && (
                        <div className="space-y-6">
                          {/* Performance */}
                          <div className="rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
                            <h3 className="mb-5 text-[11px] font-bold uppercase tracking-wider text-fg/40">
                              Performance
                            </h3>
                            <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                              <ParameterField
                                label="GPU Layers"
                                description={
                                  isCpuOnlyLlamaBackend
                                    ? "Disabled on CPU-only backends."
                                    : "Layers offloaded to GPU. 0 = CPU only."
                                }
                                value={draft.llamaGpuLayers}
                                placeholder="Auto"
                                min={0}
                                max={512}
                                step={1}
                                onChange={(v) => update({ llamaGpuLayers: v })}
                                inputMode="numeric"
                                disabled={isCpuOnlyLlamaBackend}
                              />

                              <ParameterField
                                label="Threads"
                                description="CPU threads for inference."
                                value={draft.llamaThreads}
                                placeholder="Auto"
                                min={1}
                                max={256}
                                step={1}
                                onChange={(v) => update({ llamaThreads: v })}
                                inputMode="numeric"
                              />

                              <ParameterField
                                label="Batch Threads"
                                description="CPU threads for batch processing."
                                value={draft.llamaThreadsBatch}
                                placeholder="Auto"
                                min={1}
                                max={256}
                                step={1}
                                onChange={(v) => update({ llamaThreadsBatch: v })}
                                inputMode="numeric"
                              />

                              <ParameterField
                                label="Batch Size"
                                description="Prompt processing chunk size."
                                value={draft.llamaBatchSize}
                                placeholder="512"
                                min={1}
                                max={8192}
                                step={1}
                                onChange={(v) => update({ llamaBatchSize: v })}
                                inputMode="numeric"
                              />

                              <ParameterField
                                label="Context Length"
                                description="Override context window size."
                                value={draft.contextLength}
                                placeholder="Auto"
                                min={0}
                                max={262144}
                                step={1}
                                onChange={(v) => update({ contextLength: v })}
                                inputMode="numeric"
                              />

                              <div className="space-y-2">
                                <label className="text-[13px] font-medium text-fg/80">
                                  Flash Attention
                                </label>
                                <p className="text-[11px] text-fg/40 leading-relaxed">
                                  GPU memory optimization.
                                </p>
                                <select
                                  value={draft.llamaFlashAttention ?? "auto"}
                                  onChange={(e) => {
                                    const val = e.target.value;
                                    update({
                                      llamaFlashAttention:
                                        val === "auto" ? null : (val as "enabled" | "disabled"),
                                    });
                                  }}
                                  className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg transition focus:border-fg/20 focus:outline-none"
                                >
                                  <option value="auto">Auto</option>
                                  <option value="enabled">Enabled</option>
                                  <option value="disabled">Disabled</option>
                                </select>
                              </div>
                          </div>
                        </div>

                        <div className="rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
                          <div className="flex items-start justify-between gap-4">
                            <div className="min-w-0 space-y-1.5">
                              <div className="flex items-start gap-3">
                                <div className="mt-0.5 shrink-0 text-accent/80">
                                  <Info className="h-4 w-4" />
                                </div>
                                <div className="min-w-0 space-y-1">
                                  <span className="block text-[13px] font-medium text-fg/82">
                                    Streaming
                                  </span>
                                  <span className="block text-[13px] leading-relaxed text-fg/48">
                                    Disable incremental token streaming for this llama.cpp model.
                                  </span>
                                </div>
                              </div>
                              <span className="block text-[12px] text-fg/42">
                                When off, responses are delivered only after completion.
                              </span>
                            </div>
                            <div className="flex shrink-0 items-center gap-3">
                              <span
                                className={cn(
                                  "text-[12px] font-medium transition",
                                  draft.llamaStreamingEnabled !== false
                                    ? "text-accent/80"
                                    : "text-fg/42",
                                )}
                              >
                                {draft.llamaStreamingEnabled !== false ? "On" : "Off"}
                              </span>
                              <Switch
                                id="llama-streaming-enabled-session"
                                checked={draft.llamaStreamingEnabled !== false}
                                onChange={(next) =>
                                  update({ llamaStreamingEnabled: next ? true : false })
                                }
                                aria-label="Toggle llama.cpp streaming"
                              />
                            </div>
                          </div>
                        </div>

                          {/* Sampling & Memory */}
                          <div className="rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
                            <h3 className="mb-5 text-[11px] font-bold uppercase tracking-wider text-fg/40">
                              Sampling & Memory
                            </h3>
                            <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                              <ParameterField
                                label="Min P"
                                description="Minimum probability threshold."
                                value={draft.llamaMinP}
                                placeholder="Default"
                                min={0}
                                max={1}
                                step={0.01}
                                onChange={(v) => update({ llamaMinP: v })}
                              />

                              <ParameterField
                                label="Typical P"
                                description="Typical sampling threshold."
                                value={draft.llamaTypicalP}
                                placeholder="Default"
                                min={0}
                                max={1}
                                step={0.01}
                                onChange={(v) => update({ llamaTypicalP: v })}
                              />

                              <ParameterField
                                label="DRY Multiplier"
                                description="Set above 0 to penalize repeated sequences."
                                value={draft.llamaDryMultiplier}
                                placeholder="0.80"
                                min={ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE.min}
                                max={ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE.max}
                                step={0.05}
                                onChange={(v) => update({ llamaDryMultiplier: v })}
                              />

                              <ParameterField
                                label="DRY Base"
                                description="How aggressively the DRY penalty ramps up."
                                value={draft.llamaDryBase}
                                placeholder="1.75"
                                min={ADVANCED_LLAMA_DRY_BASE_RANGE.min}
                                max={ADVANCED_LLAMA_DRY_BASE_RANGE.max}
                                step={0.05}
                                onChange={(v) => update({ llamaDryBase: v })}
                              />

                              <ParameterField
                                label="DRY Allowed Length"
                                description="Ignore repeats shorter than this many tokens."
                                value={draft.llamaDryAllowedLength}
                                placeholder="2"
                                min={ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE.min}
                                max={ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE.max}
                                step={1}
                                onChange={(v) => update({ llamaDryAllowedLength: v })}
                                inputMode="numeric"
                              />

                              <ParameterField
                                label="DRY Penalty Last N"
                                description="Use `-1` to scan the full context."
                                value={draft.llamaDryPenaltyLastN}
                                placeholder="-1"
                                min={ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE.min}
                                max={ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE.max}
                                step={1}
                                onChange={(v) => update({ llamaDryPenaltyLastN: v })}
                                inputMode="numeric"
                              />

                              <ParameterField
                                label="Seed"
                                description="Random seed. Leave blank for random."
                                value={draft.llamaSeed}
                                placeholder="Random"
                                min={0}
                                max={2147483647}
                                step={1}
                                onChange={(v) => update({ llamaSeed: v })}
                                inputMode="numeric"
                              />

                              <ParameterField
                                label="RoPE Base"
                                description="Frequency base override."
                                value={draft.llamaRopeFreqBase}
                                placeholder="Auto"
                                min={0}
                                max={1000000}
                                step={0.1}
                                onChange={(v) => update({ llamaRopeFreqBase: v })}
                              />

                              <ParameterField
                                label="RoPE Scale"
                                description="Frequency scale override."
                                value={draft.llamaRopeFreqScale}
                                placeholder="Auto"
                                min={0}
                                max={10}
                                step={0.01}
                                onChange={(v) => update({ llamaRopeFreqScale: v })}
                              />

                              <div className="space-y-2">
                                <label className="text-[13px] font-medium text-fg/80">
                                  DRY Sequence Breakers
                                </label>
                                <p className="text-[11px] text-fg/40 leading-relaxed">
                                  Comma-separated boundaries like `\n`, `:`, `"`, `*`.
                                </p>
                                <input
                                  type="text"
                                  value={draft.llamaDrySequenceBreakers?.join(", ") ?? ""}
                                  onChange={(e) => {
                                    const next = e.target.value
                                      .split(",")
                                      .map((item) => item.trim())
                                      .filter((item) => item.length > 0);
                                    update({
                                      llamaDrySequenceBreakers: next.length ? next : null,
                                    });
                                  }}
                                  placeholder={'\\n, :, ", *'}
                                  className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg transition focus:border-fg/20 focus:outline-none"
                                />
                              </div>

                              <div className="space-y-2">
                                <label className="text-[13px] font-medium text-fg/80">
                                  KV Cache Type
                                </label>
                                <p className="text-[11px] text-fg/40 leading-relaxed">
                                  Quantize KV cache to save VRAM.
                                </p>
                                <select
                                  value={draft.llamaKvType ?? "auto"}
                                  onChange={(e) => {
                                    const val = e.target.value;
                                    update({ llamaKvType: val === "auto" ? null : (val as any) });
                                  }}
                                  className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg transition focus:border-fg/20 focus:outline-none"
                                >
                                  <option value="auto">Auto</option>
                                  <option value="f16">F16</option>
                                  <option value="q8_0">Q8_0</option>
                                  <option value="q4_0">Q4_0</option>
                                </select>
                              </div>

                              <div className="space-y-2">
                                <label className="text-[13px] font-medium text-fg/80">
                                  Offload KQV
                                </label>
                                <p className="text-[11px] text-fg/40 leading-relaxed">
                                  {isCpuOnlyLlamaBackend
                                    ? "Disabled on CPU-only backends."
                                    : "KV cache & KQV ops on GPU."}
                                </p>
                                <select
                                  value={
                                    draft.llamaOffloadKqv == null
                                      ? "auto"
                                      : draft.llamaOffloadKqv
                                        ? "on"
                                        : "off"
                                  }
                                  onChange={(e) => {
                                    const val = e.target.value;
                                    update({
                                      llamaOffloadKqv: val === "auto" ? null : val === "on",
                                    });
                                  }}
                                  disabled={isCpuOnlyLlamaBackend}
                                  className={cn(
                                    "w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg transition focus:border-fg/20 focus:outline-none",
                                    isCpuOnlyLlamaBackend && "cursor-not-allowed opacity-60",
                                  )}
                                >
                                  <option value="auto">Auto</option>
                                  <option value="on">On</option>
                                  <option value="off">Off</option>
                                </select>
                              </div>

                              <div className="space-y-2">
                                <label className="text-[13px] font-medium text-fg/80">
                                  Sampler Profile
                                </label>
                                <p className="text-[11px] text-fg/40 leading-relaxed">
                                  Tuned defaults for stability or reasoning.
                                </p>
                                <select
                                  value={draft.llamaSamplerProfile ?? "balanced"}
                                  onChange={(e) => {
                                    const val = e.target.value;
                                    update({
                                      llamaSamplerProfile: val === "balanced" ? null : (val as any),
                                    });
                                  }}
                                  className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg transition focus:border-fg/20 focus:outline-none"
                                >
                                  <option value="balanced">Balanced</option>
                                  <option value="creative">Creative</option>
                                  <option value="stable">Stable</option>
                                  <option value="reasoning">Reasoning</option>
                                </select>
                              </div>

                              <div className="md:col-span-2">
                                <LlamaSamplerOrderEditor
                                  value={draft.llamaSamplerOrder}
                                  onChange={(llamaSamplerOrder) => update({ llamaSamplerOrder })}
                                />
                              </div>
                            </div>
                          </div>

                          {/* Context Info */}
                          {contextInfo && (
                            <div className="rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
                              <h3 className="mb-4 text-[11px] font-bold uppercase tracking-wider text-fg/40">
                                System Info
                              </h3>
                              <div className="grid grid-cols-2 gap-3 text-[12px] md:grid-cols-5">
                                <div>
                                  <div className="text-fg/35">Max Context</div>
                                  <div className="font-mono text-fg/75">
                                    {contextInfo.maxContextLength?.toLocaleString()}
                                  </div>
                                </div>
                                {contextInfo.recommendedContextLength != null && (
                                  <div>
                                    <div className="text-fg/35">Recommended</div>
                                    <div className="font-mono text-fg/75">
                                      {contextInfo.recommendedContextLength.toLocaleString()}
                                    </div>
                                  </div>
                                )}
                                {contextInfo.availableMemoryBytes != null && (
                                  <div>
                                    <div className="text-fg/35">Available RAM</div>
                                    <div className="font-mono text-fg/75">
                                      {(contextInfo.availableMemoryBytes / 1073741824).toFixed(1)}{" "}
                                      GB
                                    </div>
                                  </div>
                                )}
                                {contextInfo.availableVramBytes != null && (
                                  <div>
                                    <div className="text-fg/35">Available VRAM</div>
                                    <div className="font-mono text-fg/75">
                                      {(contextInfo.availableVramBytes / 1073741824).toFixed(1)} GB
                                    </div>
                                  </div>
                                )}
                                {contextInfo.modelSizeBytes != null && (
                                  <div>
                                    <div className="text-fg/35">Model Size</div>
                                    <div className="font-mono text-fg/75">
                                      {(contextInfo.modelSizeBytes / 1073741824).toFixed(1)} GB
                                    </div>
                                  </div>
                                )}
                              </div>
                            </div>
                          )}
                          {contextLoading && (
                            <div className="flex items-center gap-2 text-[12px] text-fg/40">
                              <Loader className="h-3.5 w-3.5 animate-spin" />
                              Loading context info...
                            </div>
                          )}
                        </div>
                      )}
                    </>
                  )}
                </>
              )}
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
