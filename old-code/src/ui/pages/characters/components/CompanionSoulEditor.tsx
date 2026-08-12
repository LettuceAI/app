import { useState } from "react";
import {
  Brain,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Clock,
  Compass,
  Database,
  Loader2,
  Shield,
  SlidersHorizontal,
  Sparkles,
  Square,
} from "lucide-react";
import type { CompanionConfig } from "../../../../core/storage/schemas";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { cn, interactive, radius, spacing, typography } from "../../../design-tokens";
import { Switch } from "../../../components/Switch";
import { NumberInput } from "../../../components/NumberInput";
import { normalizeCompanionConfig } from "../utils/companionDefaults";
import { SoulDirectionBottomMenu } from "./SoulDirectionBottomMenu";

type SoulTextKey =
  | "essence"
  | "traits"
  | "backstory"
  | "appearance"
  | "goals"
  | "likes"
  | "voice"
  | "relationalStyle"
  | "vulnerabilities"
  | "fears"
  | "habits"
  | "boundaries";

type AffectKey = keyof CompanionConfig["soul"]["baselineAffect"];
type RegulationKey = keyof CompanionConfig["soul"]["regulationStyle"];
type RelationshipKey = keyof CompanionConfig["relationshipDefaults"];

interface CompanionSoulEditorProps {
  companion: CompanionConfig | null | undefined;
  onChange: (next: CompanionConfig) => void;
  disabled?: boolean;
  onGenerate?: () => void;
  generating?: boolean;
  liveText?: string | null;
  stepTool?: string | null;
  onAbort?: () => void;
  generationDisabledReason?: string | null;
  modelLabel?: string | null;
  direction?: string;
  onDirectionChange?: (next: string) => void;
}

function soulStepLabelKey(tool?: string | null): TranslationKey {
  switch (tool) {
    case "set_identity":
    case "set_authored_facts":
      return "characters.soulEditor.steps.identity";
    case "set_baseline_affect":
      return "characters.soulEditor.steps.baselineAffect";
    case "set_regulation_style":
      return "characters.soulEditor.steps.regulation";
    case "set_relationship_defaults":
      return "characters.soulEditor.steps.relationship";
    case "done":
      return "characters.soulEditor.steps.finalizing";
    default:
      return "characters.soulEditor.steps.starting";
  }
}

interface TextField {
  key: SoulTextKey;
  label?: string;
  labelKey?: TranslationKey;
  placeholder?: string;
  placeholderKey?: TranslationKey;
  example?: string;
  exampleKey?: TranslationKey;
  rows: number;
}

const SOUL_TEXT_FIELDS: TextField[] = [
  {
    key: "essence",
    labelKey: "characters.soulFields.essence",
    rows: 3,
    placeholderKey: "characters.soulFields.essencePlaceholder",
    exampleKey: "characters.soulFields.essenceExample",
  },
  {
    key: "traits",
    labelKey: "characters.soulFields.traits",
    rows: 2,
    placeholderKey: "characters.soulFields.traitsPlaceholder",
    exampleKey: "characters.soulFields.traitsExample",
  },
  {
    key: "backstory",
    labelKey: "characters.soulFields.backstory",
    rows: 3,
    placeholderKey: "characters.soulFields.backstoryPlaceholder",
    exampleKey: "characters.soulFields.backstoryExample",
  },
  {
    key: "appearance",
    labelKey: "characters.soulFields.appearance",
    rows: 2,
    placeholderKey: "characters.soulFields.appearancePlaceholder",
    exampleKey: "characters.soulFields.appearanceExample",
  },
  {
    key: "goals",
    labelKey: "characters.soulFields.goals",
    rows: 2,
    placeholderKey: "characters.soulFields.goalsPlaceholder",
    exampleKey: "characters.soulFields.goalsExample",
  },
  {
    key: "likes",
    labelKey: "characters.soulFields.likes",
    rows: 2,
    placeholderKey: "characters.soulFields.likesPlaceholder",
    exampleKey: "characters.soulFields.likesExample",
  },
  {
    key: "voice",
    labelKey: "characters.soulFields.voice",
    rows: 3,
    placeholderKey: "characters.soulFields.voicePlaceholder",
    exampleKey: "characters.soulFields.voiceExample",
  },
  {
    key: "relationalStyle",
    labelKey: "characters.soulFields.relationalStyle",
    rows: 3,
    placeholderKey: "characters.soulFields.relationalStylePlaceholder",
    exampleKey: "characters.soulFields.relationalStyleExample",
  },
  {
    key: "vulnerabilities",
    labelKey: "characters.soulFields.vulnerabilities",
    rows: 2,
    placeholderKey: "characters.soulFields.vulnerabilitiesPlaceholder",
    exampleKey: "characters.soulFields.vulnerabilitiesExample",
  },
  {
    key: "fears",
    labelKey: "characters.soulFields.fears",
    rows: 2,
    placeholderKey: "characters.soulFields.fearsPlaceholder",
    exampleKey: "characters.soulFields.fearsExample",
  },
  {
    key: "habits",
    labelKey: "characters.soulFields.habits",
    rows: 2,
    placeholderKey: "characters.soulFields.habitsPlaceholder",
    exampleKey: "characters.soulFields.habitsExample",
  },
  {
    key: "boundaries",
    labelKey: "characters.soulFields.boundaries",
    rows: 2,
    placeholderKey: "characters.soulFields.boundariesPlaceholder",
    exampleKey: "characters.soulFields.boundariesExample",
  },
];

interface SliderSpec<K extends string> {
  key: K;
  labelKey: TranslationKey;
  lowKey: TranslationKey;
  highKey: TranslationKey;
}

const AFFECT_SLIDERS: SliderSpec<AffectKey>[] = [
  { key: "warmth", labelKey: "characters.soulSliders.warmth", lowKey: "characters.soulSliders.warmthLow", highKey: "characters.soulSliders.warmthHigh" },
  { key: "trust", labelKey: "characters.soulSliders.trust", lowKey: "characters.soulSliders.trustLow", highKey: "characters.soulSliders.trustHigh" },
  { key: "calm", labelKey: "characters.soulSliders.calm", lowKey: "characters.soulSliders.calmLow", highKey: "characters.soulSliders.calmHigh" },
  { key: "vulnerability", labelKey: "characters.soulSliders.vulnerability", lowKey: "characters.soulSliders.vulnerabilityLow", highKey: "characters.soulSliders.vulnerabilityHigh" },
  { key: "longing", labelKey: "characters.soulSliders.longing", lowKey: "characters.soulSliders.longingLow", highKey: "characters.soulSliders.longingHigh" },
  { key: "hurt", labelKey: "characters.soulSliders.hurt", lowKey: "characters.soulSliders.hurtLow", highKey: "characters.soulSliders.hurtHigh" },
  { key: "tension", labelKey: "characters.soulSliders.tension", lowKey: "characters.soulSliders.tensionLow", highKey: "characters.soulSliders.tensionHigh" },
  { key: "irritation", labelKey: "characters.soulSliders.irritation", lowKey: "characters.soulSliders.irritationLow", highKey: "characters.soulSliders.irritationHigh" },
  { key: "affectionIntensity", labelKey: "characters.soulSliders.affection", lowKey: "characters.soulSliders.affectionLow", highKey: "characters.soulSliders.affectionHigh" },
  { key: "reassuranceNeed", labelKey: "characters.soulSliders.reassuranceNeed", lowKey: "characters.soulSliders.reassuranceNeedLow", highKey: "characters.soulSliders.reassuranceNeedHigh" },
];

const REGULATION_SLIDERS: SliderSpec<RegulationKey>[] = [
  { key: "suppression", labelKey: "characters.soulSliders.suppression", lowKey: "characters.soulSliders.suppressionLow", highKey: "characters.soulSliders.suppressionHigh" },
  { key: "volatility", labelKey: "characters.soulSliders.volatility", lowKey: "characters.soulSliders.volatilityLow", highKey: "characters.soulSliders.volatilityHigh" },
  { key: "recoverySpeed", labelKey: "characters.soulSliders.recoverySpeed", lowKey: "characters.soulSliders.recoverySpeedLow", highKey: "characters.soulSliders.recoverySpeedHigh" },
  { key: "conflictAvoidance", labelKey: "characters.soulSliders.conflictAvoidance", lowKey: "characters.soulSliders.conflictAvoidanceLow", highKey: "characters.soulSliders.conflictAvoidanceHigh" },
  { key: "reassuranceSeeking", labelKey: "characters.soulSliders.reassuranceSeeking", lowKey: "characters.soulSliders.reassuranceSeekingLow", highKey: "characters.soulSliders.reassuranceSeekingHigh" },
  { key: "protestBehavior", labelKey: "characters.soulSliders.protestBehavior", lowKey: "characters.soulSliders.protestBehaviorLow", highKey: "characters.soulSliders.protestBehaviorHigh" },
  { key: "emotionalTransparency", labelKey: "characters.soulSliders.transparency", lowKey: "characters.soulSliders.transparencyLow", highKey: "characters.soulSliders.transparencyHigh" },
  { key: "attachmentActivation", labelKey: "characters.soulSliders.attachmentActivation", lowKey: "characters.soulSliders.attachmentActivationLow", highKey: "characters.soulSliders.attachmentActivationHigh" },
  { key: "pride", labelKey: "characters.soulSliders.pride", lowKey: "characters.soulSliders.prideLow", highKey: "characters.soulSliders.prideHigh" },
];

const RELATIONSHIP_SLIDERS: SliderSpec<RelationshipKey>[] = [
  { key: "closeness", labelKey: "characters.soulSliders.closeness", lowKey: "characters.soulSliders.closenessLow", highKey: "characters.soulSliders.closenessHigh" },
  { key: "trust", labelKey: "characters.soulSliders.relTrust", lowKey: "characters.soulSliders.relTrustLow", highKey: "characters.soulSliders.relTrustHigh" },
  { key: "affection", labelKey: "characters.soulSliders.relAffection", lowKey: "characters.soulSliders.relAffectionLow", highKey: "characters.soulSliders.relAffectionHigh" },
  { key: "tension", labelKey: "characters.soulSliders.relTension", lowKey: "characters.soulSliders.relTensionLow", highKey: "characters.soulSliders.relTensionHigh" },
];

function pct(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function summarizeAffect(
  values: CompanionConfig["soul"]["baselineAffect"],
  t: (key: TranslationKey) => string,
): string {
  const sorted = (Object.entries(values) as Array<[AffectKey, number]>)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 2)
    .map(([k]) => {
      const spec = AFFECT_SLIDERS.find((s) => s.key === k);
      return spec ? t(spec.labelKey) : k;
    });
  return sorted.join(" · ");
}

function summarizeRegulation(
  values: CompanionConfig["soul"]["regulationStyle"],
  t: (key: TranslationKey) => string,
): string {
  const sorted = (Object.entries(values) as Array<[RegulationKey, number]>)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 2)
    .map(([k]) => {
      const spec = REGULATION_SLIDERS.find((s) => s.key === k);
      return spec ? t(spec.labelKey) : k;
    });
  return sorted.join(" · ");
}

function summarizeRelationship(values: CompanionConfig["relationshipDefaults"]): string {
  return `closeness ${pct(values.closeness)} · trust ${pct(values.trust)}`;
}

const sectionLabel = cn(
  typography.label.size,
  typography.label.weight,
  typography.label.tracking,
  "uppercase text-fg/70",
);

export function CompanionSoulEditor({
  companion,
  onChange,
  disabled = false,
  onGenerate,
  generating = false,
  liveText,
  stepTool,
  onAbort,
  generationDisabledReason,
  modelLabel,
  direction = "",
  onDirectionChange,
}: CompanionSoulEditorProps) {
  const { t } = useI18n();
  const value = normalizeCompanionConfig(companion);
  const [openSection, setOpenSection] = useState<"affect" | "regulation" | "relationship" | null>(null);
  const [showExamples, setShowExamples] = useState(false);
  const [directionOpen, setDirectionOpen] = useState(false);
  const [showLiveOutput, setShowLiveOutput] = useState(false);

  const updateSoulText = (key: SoulTextKey, nextValue: string) => {
    onChange({ ...value, soul: { ...value.soul, [key]: nextValue } });
  };
  const updateAffect = (key: AffectKey, nextValue: number) => {
    onChange({
      ...value,
      soul: {
        ...value.soul,
        baselineAffect: { ...value.soul.baselineAffect, [key]: nextValue },
      },
    });
  };
  const updateRegulation = (key: RegulationKey, nextValue: number) => {
    onChange({
      ...value,
      soul: {
        ...value.soul,
        regulationStyle: { ...value.soul.regulationStyle, [key]: nextValue },
      },
    });
  };
  const updateRelationship = (key: RelationshipKey, nextValue: number) => {
    onChange({
      ...value,
      relationshipDefaults: { ...value.relationshipDefaults, [key]: nextValue },
    });
  };

  const insertExample = (field: TextField) => {
    if ((value.soul[field.key] ?? "").trim().length > 0) return;
    updateSoulText(field.key, field.exampleKey ? t(field.exampleKey) : (field.example ?? ""));
  };

  const renderSlider = <K extends string>(
    spec: SliderSpec<K>,
    sliderValue: number,
    onSliderChange: (next: number) => void,
  ) => {
    const intValue = Math.round(sliderValue * 100);
    return (
      <div key={spec.key} className={spacing.tight}>
        <div className="flex items-center justify-between gap-2">
          <span className="text-sm text-fg/80">{t(spec.labelKey)}</span>
          <span className="inline-flex items-center gap-0.5 text-[11px] text-fg/50">
            <NumberInput
              min={0}
              max={100}
              step={1}
              disabled={disabled}
              value={intValue}
              onChange={(next) => onSliderChange((next ?? 0) / 100)}
              className={cn(
                "w-9 border-b border-transparent bg-transparent text-right text-fg/70 tabular-nums outline-none",
                "hover:border-fg/15 focus:border-fg/30 focus:text-fg",
                "[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none",
                "disabled:cursor-not-allowed disabled:opacity-50",
              )}
              aria-label={`${t(spec.labelKey)} percent`}
            />
            <span aria-hidden="true">%</span>
          </span>
        </div>
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          disabled={disabled}
          value={intValue}
          onChange={(event) => onSliderChange(Number(event.target.value) / 100)}
          className="w-full accent-accent disabled:opacity-50"
        />
        <div className="flex justify-between text-[10px] text-fg/40">
          <span>{t(spec.lowKey)}</span>
          <span>{t(spec.highKey)}</span>
        </div>
      </div>
    );
  };

  const renderCollapsible = (
    id: "affect" | "regulation" | "relationship",
    Icon: typeof Brain,
    title: string,
    summary: string,
    info: string,
    body: React.ReactNode,
    iconChipClasses: string,
  ) => {
    const open = openSection === id;
    return (
      <div
        className={cn(
          "overflow-hidden border border-fg/10 bg-fg/5",
          radius.lg,
          interactive.transition.default,
        )}
      >
        <button
          type="button"
          onClick={() => setOpenSection(open ? null : id)}
          aria-expanded={open}
          className="flex w-full items-center gap-3 px-3.5 py-3 text-left transition-colors hover:bg-fg/[0.07]"
        >
          <div className={cn("rounded-lg border p-1.5", iconChipClasses)}>
            <Icon className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h3 className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>{title}</h3>
            </div>
            <p className="mt-0.5 truncate text-xs text-fg/45">{summary}</p>
          </div>
          <ChevronDown
            className={cn(
              "h-4 w-4 shrink-0 text-fg/40 transition-transform duration-150",
              open && "rotate-180",
            )}
          />
        </button>
        <div
          className={cn(
            "grid transition-[grid-template-rows] duration-200 ease-out",
            open ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
          )}
        >
          <div className="overflow-hidden">
            <div className="border-t border-fg/10 p-3.5">
              <p className={cn(typography.bodySmall.size, "mb-3 text-fg/50")}>{info}</p>
              {body}
            </div>
          </div>
        </div>
      </div>
    );
  };

  const generateBlocked = Boolean(generationDisabledReason);

  return (
    <div className={spacing.section}>
      {onGenerate && (
        <div className={cn("border border-accent/25 bg-accent/[0.06] p-4", radius.lg)}>
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 items-start gap-3">
              <div
                className={cn(
                  "flex h-9 w-9 shrink-0 items-center justify-center border border-accent/30 bg-accent/15 text-accent",
                  radius.md,
                )}
              >
                <Sparkles className="h-4 w-4" />
              </div>
              <div className="min-w-0">
                <p className={cn(typography.body.size, "font-semibold text-fg")}>
                  {t("characters.soulEditor.generateTitle")}
                </p>
                <p className={cn(typography.bodySmall.size, "mt-0.5 text-fg/55")}>
                  {generateBlocked
                    ? generationDisabledReason
                    : modelLabel
                      ? t("characters.soulEditor.generateUsingModel", { model: modelLabel })
                      : t("characters.soulEditor.generateDefaultDesc")}
                </p>
              </div>
            </div>
            {generating ? (
              <div className="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  onClick={() => setShowLiveOutput((open) => !open)}
                  className={cn(
                    "inline-flex items-center gap-1.5 border border-accent/30 bg-accent/10 px-3 py-2.5 font-medium text-accent/90",
                    typography.bodySmall.size,
                    radius.md,
                    interactive.transition.fast,
                    "hover:border-accent/45 hover:bg-accent/20",
                  )}
                >
                  <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin" />
                  <span className="max-w-[10rem] truncate">{t(soulStepLabelKey(stepTool))}</span>
                  {showLiveOutput ? (
                    <ChevronUp className="h-3.5 w-3.5 shrink-0" />
                  ) : (
                    <ChevronDown className="h-3.5 w-3.5 shrink-0" />
                  )}
                </button>
                {onAbort && (
                  <button
                    type="button"
                    onClick={onAbort}
                    className={cn(
                      "inline-flex shrink-0 items-center gap-1.5 border border-danger/35 bg-danger/10 px-4 py-2.5 font-semibold text-danger",
                      typography.bodySmall.size,
                      radius.md,
                      interactive.transition.fast,
                      interactive.active.scale,
                      "hover:border-danger/55 hover:bg-danger/20",
                    )}
                  >
                    <Square className="h-3.5 w-3.5" />
                    {t("characters.soulEditor.stop")}
                  </button>
                )}
              </div>
            ) : (
              <button
                type="button"
                onClick={onGenerate}
                disabled={disabled || generateBlocked}
                title={generationDisabledReason ?? undefined}
                className={cn(
                  "inline-flex shrink-0 items-center justify-center gap-1.5 border border-accent/40 bg-accent/20 px-4 py-2.5 font-semibold text-accent",
                  typography.bodySmall.size,
                  radius.md,
                  interactive.transition.fast,
                  interactive.active.scale,
                  "hover:border-accent/55 hover:bg-accent/30 disabled:cursor-not-allowed disabled:opacity-50",
                )}
              >
                <Sparkles className="h-4 w-4" />
                {t("characters.soulEditor.generateSoul")}
              </button>
            )}
          </div>

          {generating && showLiveOutput && (
            <div className="mt-3 border-t border-accent/15 pt-3">
              {liveText ? (
                <pre
                  className={cn(
                    "max-h-40 overflow-y-auto whitespace-pre-wrap break-words bg-base/40 p-2 text-fg/55",
                    radius.md,
                  )}
                  style={{ fontSize: "11px", lineHeight: "1.5" }}
                >
                  {liveText}
                </pre>
              ) : (
                <p className={cn(typography.bodySmall.size, "text-fg/45")}>
                  {t("characters.soulEditor.waitingForOutput")}
                </p>
              )}
            </div>
          )}

          {onDirectionChange && (
            <button
              type="button"
              onClick={() => setDirectionOpen(true)}
              className={cn(
                "mt-3 flex w-full items-center gap-2 border-t border-accent/15 pt-3 text-left",
                typography.bodySmall.size,
                interactive.transition.fast,
              )}
              title={t("characters.soulEditor.directionEditTooltip")}
            >
              <Compass
                className={cn("h-3.5 w-3.5 shrink-0", direction.trim() ? "text-info" : "text-fg/45")}
              />
              <span
                className={cn(
                  "font-medium",
                  direction.trim() ? "text-info" : "text-fg/65",
                )}
              >
                {t("characters.soulEditor.directionLabel")}
              </span>
              <span className="min-w-0 flex-1 truncate text-fg/40">
                {direction.trim()
                  ? direction.trim()
                  : t("characters.soulEditor.directionOptional")}
              </span>
              <ChevronRight className="h-3.5 w-3.5 shrink-0 text-fg/30" />
            </button>
          )}
        </div>
      )}

      <div className={spacing.field}>
        <div className="flex items-center justify-between">
          <label className={sectionLabel}>{t("characters.soulEditor.identityLabel")}</label>
          <button
            type="button"
            onClick={() => setShowExamples((v) => !v)}
            className={cn(typography.caption.size, "text-fg/55 hover:text-fg")}
          >
            {showExamples
              ? t("characters.soulEditor.hideExamples")
              : t("characters.soulEditor.showExamples")}
          </button>
        </div>
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          {SOUL_TEXT_FIELDS.map((field) => {
            const fieldValue = value.soul[field.key] ?? "";
            const filled = fieldValue.trim().length > 0;
            return (
              <div
                key={field.key}
                className={cn(spacing.field, field.key === "essence" && "lg:col-span-2")}
              >
                <div className="flex items-center justify-between">
                  <label className="text-xs font-medium text-fg/70">
                    {field.labelKey ? t(field.labelKey) : field.label}
                  </label>
                  {showExamples && !filled && (
                    <button
                      type="button"
                      onClick={() => insertExample(field)}
                      className="text-[11px] text-accent/80 hover:text-accent"
                    >
                      {t("characters.soulEditor.insertExample")}
                    </button>
                  )}
                </div>
                <textarea
                  value={fieldValue}
                  onChange={(event) => updateSoulText(field.key, event.target.value)}
                  rows={field.rows}
                  disabled={disabled}
                  placeholder={field.placeholderKey ? t(field.placeholderKey) : field.placeholder}
                  className={cn(
                    "w-full resize-none border bg-surface-el/20 px-4 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl",
                    radius.md,
                    interactive.transition.default,
                    "focus:bg-surface-el/30 focus:outline-none disabled:cursor-not-allowed",
                    filled
                      ? "border-fg/20 focus:border-fg/40"
                      : "border-fg/10 focus:border-fg/30",
                  )}
                />
                {showExamples && (
                  <p className={cn(typography.bodySmall.size, "italic text-fg/40")}>
                    {t("characters.soulEditor.exampleEg", {
                      example: field.exampleKey ? t(field.exampleKey) : (field.example ?? ""),
                    })}
                  </p>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <div className={spacing.field}>
        <label className={sectionLabel}>{t("characters.soulEditor.fineTuneLabel")}</label>
        <div className={spacing.item}>
          {renderCollapsible(
            "affect",
            Brain,
            t("characters.soulEditor.baselineAffect"),
            summarizeAffect(value.soul.baselineAffect, t),
            "How they feel by default — the emotional waterline before anything happens.",
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              {AFFECT_SLIDERS.map((spec) =>
                renderSlider(spec, value.soul.baselineAffect[spec.key], (next) =>
                  updateAffect(spec.key, next),
                ),
              )}
            </div>,
            "border-info/30 bg-info/10 text-info",
          )}
          {renderCollapsible(
            "regulation",
            SlidersHorizontal,
            t("characters.soulEditor.regulationStyle"),
            summarizeRegulation(value.soul.regulationStyle, t),
            "How they handle and express what they feel — venting vs. burying.",
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              {REGULATION_SLIDERS.map((spec) =>
                renderSlider(spec, value.soul.regulationStyle[spec.key], (next) =>
                  updateRegulation(spec.key, next),
                ),
              )}
            </div>,
            "border-warning/30 bg-warning/10 text-warning",
          )}
          {renderCollapsible(
            "relationship",
            Shield,
            t("characters.soulEditor.relationshipDefaults"),
            summarizeRelationship(value.relationshipDefaults),
            t("characters.soulEditor.relationshipDefaultsInfo"),
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              {RELATIONSHIP_SLIDERS.map((spec) =>
                renderSlider(spec, value.relationshipDefaults[spec.key], (next) =>
                  updateRelationship(spec.key, next),
                ),
              )}
            </div>,
            "border-secondary/30 bg-secondary/10 text-secondary",
          )}
        </div>
      </div>

      <div className={spacing.field}>
        <label className={sectionLabel}>{t("characters.soulEditor.companionContextLabel")}</label>
        <div className={spacing.item}>
          <div
            className={cn(
              "flex items-start justify-between gap-3 border border-fg/10 bg-surface-el/40 p-4",
              radius.md,
            )}
          >
            <div className="flex min-w-0 items-start gap-3">
              <div
                className={cn(
                  "flex h-8 w-8 shrink-0 items-center justify-center border border-fg/10 bg-fg/5 text-fg/75",
                  radius.full,
                )}
              >
                <Clock className="h-4 w-4" />
              </div>
              <div className="min-w-0">
                <p className={cn(typography.body.size, "font-semibold text-fg")}>
                  {t("characters.soulEditor.timeAwarenessTitle")}
                </p>
                <p className={cn(typography.bodySmall.size, "mt-1 text-fg/55")}>
                  {t("characters.soulEditor.timeAwarenessDesc")}
                </p>
              </div>
            </div>
            <Switch
              checked={value.timeAwareness}
              onChange={(checked) =>
                onChange({ ...value, timeAwareness: checked })
              }
              disabled={disabled}
              aria-label={t("characters.soulEditor.timeAwarenessAria")}
            />
          </div>

          <div
            className={cn(
              "flex items-start justify-between gap-3 border border-fg/10 bg-surface-el/40 p-4",
              radius.md,
            )}
          >
            <div className="flex min-w-0 items-start gap-3">
              <div
                className={cn(
                  "flex h-8 w-8 shrink-0 items-center justify-center border border-fg/10 bg-fg/5 text-fg/75",
                  radius.full,
                )}
              >
                <Database className="h-4 w-4" />
              </div>
              <div className="min-w-0">
                <p className={cn(typography.body.size, "font-semibold text-fg")}>
                  {t("characters.soulEditor.sharedMemoryTitle")}
                </p>
                <p className={cn(typography.bodySmall.size, "mt-1 text-fg/55")}>
                  {t("characters.soulEditor.sharedMemoryDesc")}
                </p>
              </div>
            </div>
            <Switch
              checked={value.memory.sharedAcrossSessions}
              onChange={(checked) =>
                onChange({
                  ...value,
                  memory: {
                    ...value.memory,
                    sharedAcrossSessions: checked,
                  },
                })
              }
              disabled={disabled}
              aria-label={t("characters.soulEditor.sharedMemoryAria")}
            />
          </div>
        </div>
      </div>

      {onDirectionChange && (
        <SoulDirectionBottomMenu
          isOpen={directionOpen}
          onClose={() => setDirectionOpen(false)}
          direction={direction}
          onDirectionChange={onDirectionChange}
        />
      )}
    </div>
  );
}
