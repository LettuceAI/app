import { RotateCcw } from "lucide-react";
import type { ReactNode } from "react";

import {
  LLAMA_SAMPLER_PROFILE_VALUES,
  providerSupportsParameter,
  type FeatureGenerationSettings,
  type LlamaSamplerProfile,
} from "../../core/storage/schemas";
import { useI18n, type TranslationKey } from "../../core/i18n/context";
import { cn } from "../design-tokens";
import {
  ADVANCED_FREQUENCY_PENALTY_RANGE,
  ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE,
  ADVANCED_LLAMA_DRY_BASE_RANGE,
  ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE,
  ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE,
  ADVANCED_LLAMA_REPEAT_PENALTY_RANGE,
  ADVANCED_LLAMA_N_PEN_RANGE,
  ADVANCED_LLAMA_SEED_RANGE,
  ADVANCED_LLAMA_XTC_PROBABILITY_RANGE,
  ADVANCED_LLAMA_XTC_THRESHOLD_RANGE,
  ADVANCED_MAX_TOKENS_RANGE,
  ADVANCED_PRESENCE_PENALTY_RANGE,
  ADVANCED_TEMPERATURE_RANGE,
  ADVANCED_TOP_K_RANGE,
  ADVANCED_TOP_P_RANGE,
} from "./AdvancedModelSettingsForm";
import { LlamaSamplerOrderEditor } from "./LlamaSamplerOrderEditor";
import { NumberInput } from "./NumberInput";

const numberInputClassName =
  "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none";
const selectInputClassName =
  "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg transition focus:border-fg/30 focus:outline-none";
const textInputClassName =
  "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none";

const GENERIC_NUMBER_FIELDS = [
  {
    key: "temperature",
    labelKey: "featureGeneration.temperature",
    range: ADVANCED_TEMPERATURE_RANGE,
    decimals: 2,
    step: 0.05,
    supportKey: "temperature",
  },
  {
    key: "topP",
    labelKey: "featureGeneration.topP",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.05,
    supportKey: "topP",
  },
  {
    key: "topK",
    labelKey: "featureGeneration.topK",
    range: ADVANCED_TOP_K_RANGE,
    decimals: 0,
    step: 1,
    supportKey: "topK",
  },
  {
    key: "maxOutputTokens",
    labelKey: "featureGeneration.maxOutputTokens",
    range: ADVANCED_MAX_TOKENS_RANGE,
    decimals: 0,
    step: 128,
    supportKey: "maxOutputTokens",
  },
  {
    key: "frequencyPenalty",
    labelKey: "featureGeneration.frequencyPenalty",
    range: ADVANCED_FREQUENCY_PENALTY_RANGE,
    decimals: 2,
    step: 0.05,
    supportKey: "frequencyPenalty",
  },
  {
    key: "presencePenalty",
    labelKey: "featureGeneration.presencePenalty",
    range: ADVANCED_PRESENCE_PENALTY_RANGE,
    decimals: 2,
    step: 0.05,
    supportKey: "presencePenalty",
  },
] as const;

const LLAMA_NUMBER_FIELDS = [
  {
    key: "llamaMinP",
    labelKey: "featureGeneration.minP",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "llamaTypicalP",
    labelKey: "featureGeneration.typicalP",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "llamaRepeatPenalty",
    labelKey: "featureGeneration.repeatPenalty",
    range: ADVANCED_LLAMA_REPEAT_PENALTY_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "llamaNPenRange",
    labelKey: "featureGeneration.penaltyRange",
    range: ADVANCED_LLAMA_N_PEN_RANGE,
    decimals: 0,
    step: 1,
  },
  {
    key: "llamaXtcProbability",
    labelKey: "featureGeneration.xtcProbability",
    range: ADVANCED_LLAMA_XTC_PROBABILITY_RANGE,
    decimals: 2,
    step: 0.05,
  },
  {
    key: "llamaXtcThreshold",
    labelKey: "featureGeneration.xtcThreshold",
    range: ADVANCED_LLAMA_XTC_THRESHOLD_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "llamaDryMultiplier",
    labelKey: "featureGeneration.dryMultiplier",
    range: ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE,
    decimals: 2,
    step: 0.05,
  },
  {
    key: "llamaDryBase",
    labelKey: "featureGeneration.dryBase",
    range: ADVANCED_LLAMA_DRY_BASE_RANGE,
    decimals: 2,
    step: 0.05,
  },
  {
    key: "llamaDryAllowedLength",
    labelKey: "featureGeneration.dryAllowedLength",
    range: ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE,
    decimals: 0,
    step: 1,
  },
  {
    key: "llamaDryPenaltyLastN",
    labelKey: "featureGeneration.dryPenaltyLastN",
    range: ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE,
    decimals: 0,
    step: 1,
  },
  {
    key: "llamaSeed",
    labelKey: "featureGeneration.seed",
    range: ADVANCED_LLAMA_SEED_RANGE,
    decimals: 0,
    step: 1,
  },
] as const;

const OLLAMA_NUMBER_FIELDS = [
  {
    key: "ollamaMinP",
    labelKey: "featureGeneration.minP",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "ollamaTypicalP",
    labelKey: "featureGeneration.typicalP",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "ollamaTfsZ",
    labelKey: "featureGeneration.tfsZ",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "ollamaRepeatPenalty",
    labelKey: "featureGeneration.repeatPenalty",
    range: ADVANCED_LLAMA_REPEAT_PENALTY_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "ollamaMirostat",
    labelKey: "featureGeneration.mirostat",
    range: { min: 0, max: 2 },
    decimals: 0,
    step: 1,
  },
  {
    key: "ollamaMirostatTau",
    labelKey: "featureGeneration.mirostatTau",
    range: { min: 0, max: 10 },
    decimals: 1,
    step: 0.5,
  },
  {
    key: "ollamaMirostatEta",
    labelKey: "featureGeneration.mirostatEta",
    range: ADVANCED_TOP_P_RANGE,
    decimals: 2,
    step: 0.01,
  },
  {
    key: "ollamaSeed",
    labelKey: "featureGeneration.seed",
    range: ADVANCED_LLAMA_SEED_RANGE,
    decimals: 0,
    step: 1,
  },
] as const;

type NumericKey =
  | (typeof GENERIC_NUMBER_FIELDS)[number]["key"]
  | (typeof LLAMA_NUMBER_FIELDS)[number]["key"]
  | (typeof OLLAMA_NUMBER_FIELDS)[number]["key"];

type NumberFieldSpec = {
  key: NumericKey;
  labelKey: string;
  range: { min: number; max: number };
  decimals: number;
  step: number;
};

function clamp(value: number, range: { min: number; max: number }): number {
  return Math.min(range.max, Math.max(range.min, value));
}

function hasAnyValue(value: FeatureGenerationSettings): boolean {
  return Object.values(value).some((entry) => entry !== null && entry !== undefined);
}

function SectionHeader({ title, action }: { title: string; action?: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
        <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
          {title}
        </span>
      </div>
      {action}
    </div>
  );
}

function FieldShell({
  label,
  description,
  readout,
  rangeLabels,
  children,
}: {
  label: string;
  description: string;
  readout: string;
  rangeLabels?: [string, string];
  children: ReactNode;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="space-y-0.5">
          <span className="block text-[13px] font-medium text-fg/70">{label}</span>
          <span className="block text-[13px] text-fg/40">{description}</span>
        </div>
        <span className="font-mono text-[13px] text-fg/55">{readout}</span>
      </div>
      {children}
      {rangeLabels ? (
        <div className="mt-1 flex justify-between px-0.5 text-[13px] text-fg/30">
          <span>{rangeLabels[0]}</span>
          <span>{rangeLabels[1]}</span>
        </div>
      ) : null}
    </div>
  );
}

function listToText(value: string[] | null | undefined): string {
  return (value ?? []).join(", ");
}

function textToList(value: string): string[] | null {
  const parts = value
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
  return parts.length > 0 ? parts : null;
}

const FIELD_GRID_CLASS =
  "grid grid-cols-1 gap-x-6 gap-y-8 md:grid-cols-2 xl:grid-cols-3 xl:gap-x-8";

export function FeatureGenerationSettingsEditor({
  value,
  onChange,
  providerId,
  defaults,
  className,
}: {
  value: FeatureGenerationSettings | null | undefined;
  onChange: (next: FeatureGenerationSettings | null) => void;
  providerId?: string | null;
  defaults: { temperature: number; topP: number; maxOutputTokens?: number };
  className?: string;
}) {
  const { t } = useI18n();
  const current = value ?? {};
  const isLlama = providerId === "llamacpp";
  const isOllama = providerId === "ollama";
  const hasOverrides = !!value && hasAnyValue(current);

  const set = (patch: Partial<FeatureGenerationSettings>) => {
    const next: FeatureGenerationSettings = { ...current, ...patch };
    for (const key of Object.keys(next) as (keyof FeatureGenerationSettings)[]) {
      if (next[key] === null || next[key] === undefined) delete next[key];
    }
    onChange(hasAnyValue(next) ? next : null);
  };

  const supportsGeneric = (param: string) => {
    if (!providerId || isLlama || isOllama) return true;
    return providerSupportsParameter(providerId, param as never);
  };

  const defaultFor = (key: NumericKey): string | null => {
    if (key === "temperature") return defaults.temperature.toFixed(2);
    if (key === "topP") return defaults.topP.toFixed(2);
    if (key === "maxOutputTokens" && defaults.maxOutputTokens !== undefined) {
      return String(defaults.maxOutputTokens);
    }
    return null;
  };

  const numberField = (field: NumberFieldSpec) => {
    const raw = current[field.key];
    const featureDefault = defaultFor(field.key);
    const readout =
      typeof raw === "number"
        ? raw.toFixed(field.decimals)
        : (featureDefault ?? t("common.labels.auto"));
    return (
      <FieldShell
        key={field.key}
        label={t(field.labelKey as TranslationKey)}
        description={
          featureDefault
            ? t("featureGeneration.featureDefaultHelper", { value: featureDefault })
            : t("featureGeneration.inheritsHelper")
        }
        readout={readout}
        rangeLabels={[String(field.range.min), String(field.range.max)]}
      >
        <NumberInput
          value={typeof raw === "number" ? raw : null}
          onChange={(next) =>
            set({
              [field.key]: next === null ? null : clamp(next, field.range),
            } as Partial<FeatureGenerationSettings>)
          }
          min={field.range.min}
          max={field.range.max}
          step={field.step}
          decimals={field.decimals}
          placeholder={featureDefault ?? t("common.labels.auto")}
          className={numberInputClassName}
        />
      </FieldShell>
    );
  };

  return (
    <div className={cn("space-y-8", className)}>
      <SectionHeader
        title={t("featureGeneration.samplingSection")}
        action={
          <button
            type="button"
            onClick={() => onChange(null)}
            disabled={!hasOverrides}
            className={cn(
              "inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11px] font-medium transition-colors",
              !hasOverrides
                ? "cursor-default border-fg/5 bg-transparent text-fg/25"
                : "border-fg/10 bg-fg/5 text-fg/65 hover:border-fg/20 hover:bg-fg/8 hover:text-fg",
            )}
          >
            <RotateCcw size={12} />
            {t("featureGeneration.reset")}
          </button>
        }
      />
      <div className={FIELD_GRID_CLASS}>
        {GENERIC_NUMBER_FIELDS.filter((field) => supportsGeneric(field.supportKey)).map(
          numberField,
        )}
      </div>

      {isLlama && (
        <>
          <SectionHeader title={t("featureGeneration.llamaSection")} />
          <div className={FIELD_GRID_CLASS}>
            <FieldShell
              label={t("featureGeneration.samplerProfile")}
              description={t("featureGeneration.inheritsHelper")}
              readout={
                current.llamaSamplerProfile
                  ? t(
                      `editModel.samplerProfile.${current.llamaSamplerProfile}` as TranslationKey,
                    )
                  : t("common.labels.default")
              }
            >
              <select
                value={current.llamaSamplerProfile ?? ""}
                onChange={(event) =>
                  set({
                    llamaSamplerProfile: (event.target.value ||
                      null) as LlamaSamplerProfile | null,
                  })
                }
                className={selectInputClassName}
              >
                <option value="" className="bg-[#16171d]">
                  {t("featureGeneration.modelDefault")}
                </option>
                {LLAMA_SAMPLER_PROFILE_VALUES.map((profile) => (
                  <option key={profile} value={profile} className="bg-[#16171d]">
                    {t(`editModel.samplerProfile.${profile}` as TranslationKey)}
                  </option>
                ))}
              </select>
            </FieldShell>
            {LLAMA_NUMBER_FIELDS.map(numberField)}
            <FieldShell
              label={t("featureGeneration.drySequenceBreakers")}
              description={t("featureGeneration.listHelper")}
              readout={
                current.llamaDrySequenceBreakers?.length
                  ? String(current.llamaDrySequenceBreakers.length)
                  : t("common.labels.default")
              }
            >
              <input
                value={listToText(current.llamaDrySequenceBreakers)}
                onChange={(event) =>
                  set({ llamaDrySequenceBreakers: textToList(event.target.value) })
                }
                placeholder={t("featureGeneration.modelDefault")}
                className={textInputClassName}
              />
            </FieldShell>
          </div>
          <LlamaSamplerOrderEditor
            value={current.llamaSamplerOrder ?? null}
            onChange={(next) => set({ llamaSamplerOrder: next })}
          />
        </>
      )}

      {isOllama && (
        <>
          <SectionHeader title={t("featureGeneration.ollamaSection")} />
          <div className={FIELD_GRID_CLASS}>
            {OLLAMA_NUMBER_FIELDS.map(numberField)}
            <FieldShell
              label={t("featureGeneration.stopSequences")}
              description={t("featureGeneration.listHelper")}
              readout={
                current.ollamaStop?.length
                  ? String(current.ollamaStop.length)
                  : t("common.labels.default")
              }
            >
              <input
                value={listToText(current.ollamaStop)}
                onChange={(event) => set({ ollamaStop: textToList(event.target.value) })}
                placeholder={t("featureGeneration.modelDefault")}
                className={textInputClassName}
              />
            </FieldShell>
          </div>
        </>
      )}
    </div>
  );
}
