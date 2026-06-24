import { useEffect, useMemo, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Brain,
  Check,
  ChevronDown,
  Compass,
  Loader2,
  Shield,
  SlidersHorizontal,
  Sparkles,
  X,
} from "lucide-react";
import type { CompanionConfig } from "../../../../core/storage/schemas";
import { cn, interactive, radius, spacing, typography } from "../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { mergeCompanionSoulDraft } from "../../../../core/companion/soul";
import { normalizeCompanionConfig } from "../utils/companionDefaults";

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

interface SoulGenerationReviewOverlayProps {
  isOpen: boolean;
  baseline: CompanionConfig;
  draft: Partial<CompanionConfig> | null;
  direction: string;
  onDirectionChange: (next: string) => void;
  onApply: (next: CompanionConfig) => void;
  onCancel: () => void;
  onRegenerate: () => void;
  regenerating?: boolean;
}

const TEXT_FIELDS: Array<{
  key: SoulTextKey;
  label?: string;
  labelKey?: TranslationKey;
  rows: number;
}> = [
  { key: "essence", labelKey: "characters.soulFields.essence", rows: 3 },
  { key: "traits", labelKey: "characters.soulFields.traits", rows: 2 },
  { key: "backstory", labelKey: "characters.soulFields.backstory", rows: 3 },
  { key: "appearance", labelKey: "characters.soulFields.appearance", rows: 2 },
  { key: "goals", labelKey: "characters.soulFields.goals", rows: 2 },
  { key: "likes", labelKey: "characters.soulFields.likes", rows: 2 },
  { key: "voice", labelKey: "characters.soulFields.voice", rows: 3 },
  { key: "relationalStyle", labelKey: "characters.soulFields.relationalStyle", rows: 3 },
  { key: "vulnerabilities", labelKey: "characters.soulFields.vulnerabilities", rows: 2 },
  { key: "fears", labelKey: "characters.soulFields.fears", rows: 2 },
  { key: "habits", labelKey: "characters.soulFields.habits", rows: 2 },
  { key: "boundaries", labelKey: "characters.soulFields.boundaries", rows: 2 },
];

const AFFECT_LABELS: Record<AffectKey, [TranslationKey, TranslationKey, TranslationKey]> = {
  warmth: ["characters.soulSliders.warmth", "characters.soulSliders.warmthLow", "characters.soulSliders.warmthHigh"],
  trust: ["characters.soulSliders.trust", "characters.soulSliders.trustLow", "characters.soulSliders.trustHigh"],
  calm: ["characters.soulSliders.calm", "characters.soulSliders.calmLow", "characters.soulSliders.calmHigh"],
  vulnerability: ["characters.soulSliders.vulnerability", "characters.soulSliders.vulnerabilityLow", "characters.soulSliders.vulnerabilityHigh"],
  longing: ["characters.soulSliders.longing", "characters.soulSliders.longingLow", "characters.soulSliders.longingHigh"],
  hurt: ["characters.soulSliders.hurt", "characters.soulSliders.hurtLow", "characters.soulSliders.hurtHigh"],
  tension: ["characters.soulSliders.tension", "characters.soulSliders.tensionLow", "characters.soulSliders.tensionHigh"],
  irritation: ["characters.soulSliders.irritation", "characters.soulSliders.irritationLow", "characters.soulSliders.irritationHigh"],
  affectionIntensity: ["characters.soulSliders.affection", "characters.soulSliders.affectionLow", "characters.soulSliders.affectionHigh"],
  reassuranceNeed: ["characters.soulSliders.reassuranceNeed", "characters.soulSliders.reassuranceNeedLow", "characters.soulSliders.reassuranceNeedHigh"],
};

const REGULATION_LABELS: Record<RegulationKey, [TranslationKey, TranslationKey, TranslationKey]> = {
  suppression: ["characters.soulSliders.suppression", "characters.soulSliders.suppressionLow", "characters.soulSliders.suppressionHigh"],
  volatility: ["characters.soulSliders.volatility", "characters.soulSliders.volatilityLow", "characters.soulSliders.volatilityHigh"],
  recoverySpeed: ["characters.soulSliders.recoverySpeed", "characters.soulSliders.recoverySpeedLow", "characters.soulSliders.recoverySpeedHigh"],
  conflictAvoidance: ["characters.soulSliders.conflictAvoidance", "characters.soulSliders.conflictAvoidanceLow", "characters.soulSliders.conflictAvoidanceHigh"],
  reassuranceSeeking: ["characters.soulSliders.reassuranceSeeking", "characters.soulSliders.reassuranceSeekingLow", "characters.soulSliders.reassuranceSeekingHigh"],
  protestBehavior: ["characters.soulSliders.protestBehavior", "characters.soulSliders.protestBehaviorLow", "characters.soulSliders.protestBehaviorHigh"],
  emotionalTransparency: ["characters.soulSliders.transparency", "characters.soulSliders.transparencyLow", "characters.soulSliders.transparencyHigh"],
  attachmentActivation: ["characters.soulSliders.attachmentActivation", "characters.soulSliders.attachmentActivationLow", "characters.soulSliders.attachmentActivationHigh"],
  pride: ["characters.soulSliders.pride", "characters.soulSliders.prideLow", "characters.soulSliders.prideHigh"],
};

const RELATIONSHIP_LABELS: Record<RelationshipKey, [TranslationKey, TranslationKey, TranslationKey]> = {
  closeness: ["characters.soulSliders.closeness", "characters.soulSliders.closenessLow", "characters.soulSliders.closenessHigh"],
  trust: ["characters.soulSliders.relTrust", "characters.soulSliders.relTrustLow", "characters.soulSliders.relTrustHigh"],
  affection: ["characters.soulSliders.relAffection", "characters.soulSliders.relAffectionLow", "characters.soulSliders.relAffectionHigh"],
  tension: ["characters.soulSliders.relTension", "characters.soulSliders.relTensionLow", "characters.soulSliders.relTensionHigh"],
};

function pct(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function arrow(delta: number): string {
  if (delta > 0.02) return "↑";
  if (delta < -0.02) return "↓";
  return "→";
}

function countSliderChanges(
  baseline: Record<string, number>,
  draft: Record<string, number>,
): number {
  let n = 0;
  for (const key of Object.keys(baseline)) {
    if (Math.abs((draft[key] ?? baseline[key]) - baseline[key]) >= 0.02) n += 1;
  }
  return n;
}

function countTextChanges(
  baseline: CompanionConfig["soul"],
  draft: CompanionConfig["soul"],
): number {
  let n = 0;
  for (const { key } of TEXT_FIELDS) {
    if ((baseline[key] ?? "").trim() !== (draft[key] ?? "").trim()) n += 1;
  }
  return n;
}

const sectionLabel = cn(
  typography.label.size,
  typography.label.weight,
  typography.label.tracking,
  "uppercase text-fg/55",
);

export function SoulGenerationReviewOverlay({
  isOpen,
  baseline,
  draft,
  direction,
  onDirectionChange,
  onApply,
  onCancel,
  onRegenerate,
  regenerating = false,
}: SoulGenerationReviewOverlayProps) {
  const { t } = useI18n();
  const [working, setWorking] = useState<CompanionConfig>(() =>
    draft ? mergeCompanionSoulDraft(baseline, draft) : normalizeCompanionConfig(baseline),
  );
  const [openSection, setOpenSection] = useState<"affect" | "regulation" | "relationship" | null>(null);
  const [directionOpen, setDirectionOpen] = useState(false);

  // When a fresh draft arrives, replace working state with the merged draft.
  useEffect(() => {
    if (!isOpen) return;
    if (draft) {
      setWorking(mergeCompanionSoulDraft(baseline, draft));
    } else {
      setWorking(normalizeCompanionConfig(baseline));
    }
  }, [draft, baseline, isOpen]);

  const textChanges = useMemo(
    () => countTextChanges(baseline.soul, working.soul),
    [baseline.soul, working.soul],
  );
  const affectChanges = useMemo(
    () =>
      countSliderChanges(
        baseline.soul.baselineAffect as Record<string, number>,
        working.soul.baselineAffect as Record<string, number>,
      ),
    [baseline.soul.baselineAffect, working.soul.baselineAffect],
  );
  const regulationChanges = useMemo(
    () =>
      countSliderChanges(
        baseline.soul.regulationStyle as Record<string, number>,
        working.soul.regulationStyle as Record<string, number>,
      ),
    [baseline.soul.regulationStyle, working.soul.regulationStyle],
  );
  const relationshipChanges = useMemo(
    () =>
      countSliderChanges(
        baseline.relationshipDefaults as Record<string, number>,
        working.relationshipDefaults as Record<string, number>,
      ),
    [baseline.relationshipDefaults, working.relationshipDefaults],
  );
  const totalChanges = textChanges + affectChanges + regulationChanges + relationshipChanges;

  const updateText = (key: SoulTextKey, value: string) => {
    setWorking((prev) => ({ ...prev, soul: { ...prev.soul, [key]: value } }));
  };
  const updateAffect = (key: AffectKey, value: number) => {
    setWorking((prev) => ({
      ...prev,
      soul: {
        ...prev.soul,
        baselineAffect: { ...prev.soul.baselineAffect, [key]: value },
      },
    }));
  };
  const updateRegulation = (key: RegulationKey, value: number) => {
    setWorking((prev) => ({
      ...prev,
      soul: {
        ...prev.soul,
        regulationStyle: { ...prev.soul.regulationStyle, [key]: value },
      },
    }));
  };
  const updateRelationship = (key: RelationshipKey, value: number) => {
    setWorking((prev) => ({
      ...prev,
      relationshipDefaults: { ...prev.relationshipDefaults, [key]: value },
    }));
  };

  const renderSlider = (
    key: string,
    label: [TranslationKey, TranslationKey, TranslationKey],
    value: number,
    onChange: (next: number) => void,
    baselineValue: number,
  ) => {
    const changed = Math.abs(value - baselineValue) >= 0.02;
    const changeColor = value >= baselineValue ? "text-accent" : "text-danger";
    return (
      <div key={key} className={spacing.tight}>
        <div className="flex items-center justify-between">
          <span className="text-sm text-fg/80">{t(label[0])}</span>
          <span className={cn("text-[11px]", changed ? changeColor : "text-fg/45")}>
            {changed ? (
              <>
                {pct(baselineValue)}{" "}
                <span className="text-fg/40">{arrow(value - baselineValue)}</span>{" "}
                <span className={cn("font-semibold", changeColor)}>{pct(value)}</span>
              </>
            ) : (
              pct(value)
            )}
          </span>
        </div>
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={Math.round(value * 100)}
          onChange={(event) => onChange(Number(event.target.value) / 100)}
          className="w-full accent-accent"
        />
        <div className="flex justify-between text-[10px] text-fg/40">
          <span>{t(label[1])}</span>
          <span>{t(label[2])}</span>
        </div>
      </div>
    );
  };

  const renderCollapsible = (
    id: "affect" | "regulation" | "relationship",
    Icon: typeof Brain,
    title: string,
    changeCount: number,
    body: React.ReactNode,
    iconChip: string,
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
          <div className={cn("p-1.5 border", radius.md, iconChip)}>
            <Icon className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>{title}</h3>
            <p className="mt-0.5 text-xs text-fg/45">
              {changeCount === 0
                ? "Unchanged"
                : `${changeCount} change${changeCount === 1 ? "" : "s"}`}
            </p>
          </div>
          {changeCount > 0 && (
            <span
              className={cn(
                "border border-accent/30 bg-accent/10 px-2 py-0.5 text-[10px] font-semibold text-accent",
                radius.full,
              )}
            >
              {changeCount}
            </span>
          )}
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
            <div className="border-t border-fg/10 p-3.5">{body}</div>
          </div>
        </div>
      </div>
    );
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="fixed inset-0 z-30 bg-black/40 backdrop-blur-sm"
            onClick={onCancel}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          />
          <motion.div
            initial={{ y: "100%" }}
            animate={{ y: 0 }}
            exit={{ y: "100%" }}
            transition={{ type: "spring", damping: 28, stiffness: 260 }}
            className={cn(
              "fixed inset-x-0 bottom-0 top-[max(48px,calc(72px+env(safe-area-inset-top)))] z-40 flex flex-col overflow-hidden border-t border-fg/15 bg-surface shadow-2xl",
              "rounded-t-2xl",
            )}
          >
            <div className="flex items-center gap-3 border-b border-fg/10 px-4 py-3">
              <div className={cn("border border-accent/30 bg-accent/10 p-1.5", radius.md)}>
                <Sparkles className="h-4 w-4 text-accent" />
              </div>
              <div className="min-w-0 flex-1">
                <div className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>
                  {t("characters.soulReview.reviewTitle")}
                </div>
                <div className={cn(typography.bodySmall.size, "text-fg/50")}>
                  {totalChanges === 0
                    ? t("characters.soulReview.noDifferences")
                    : `${totalChanges} change${totalChanges === 1 ? "" : "s"} — edit anything before applying.`}
                </div>
              </div>
              <button
                type="button"
                onClick={onCancel}
                className={cn(
                  "p-1.5 text-fg/45 hover:bg-fg/5 hover:text-fg",
                  radius.md,
                  interactive.transition.fast,
                )}
                aria-label={t("characters.soulReview.close")}
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div className={cn("flex-1 overflow-y-auto px-4 py-4", spacing.section)}>
              <div className="mx-auto w-full max-w-2xl space-y-5">
                <div className={spacing.field}>
                  <div className="flex items-center justify-between">
                    <label className={sectionLabel}>{t("characters.soulReview.identityLabel")}</label>
                    {textChanges > 0 && (
                      <span className={cn(typography.caption.size, "text-accent/80")}>
                        {t("characters.soulReview.nEdited", { count: textChanges })}
                      </span>
                    )}
                  </div>
                  <div className="grid grid-cols-1 gap-4">
                    {TEXT_FIELDS.map((field) => {
                      const value = working.soul[field.key] ?? "";
                      const baseValue = (baseline.soul[field.key] ?? "").trim();
                      const changed = baseValue !== value.trim();
                      return (
                        <div key={field.key} className={spacing.field}>
                          <div className="flex items-center justify-between">
                            <label className="text-xs font-medium text-fg/70">
                              {field.labelKey ? t(field.labelKey) : field.label}
                            </label>
                            {changed && (
                              <span className="text-[10px] font-semibold uppercase tracking-wider text-accent/80">
                                {t("characters.soulReview.edited")}
                              </span>
                            )}
                          </div>
                          <textarea
                            value={value}
                            onChange={(e) => updateText(field.key, e.target.value)}
                            rows={field.rows}
                            className={cn(
                              "w-full resize-none border bg-surface-el/20 px-4 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl",
                              radius.md,
                              interactive.transition.default,
                              "focus:bg-surface-el/30 focus:outline-none",
                              changed
                                ? "border-accent/30 focus:border-accent/45"
                                : "border-fg/10 focus:border-fg/30",
                            )}
                          />
                        </div>
                      );
                    })}
                  </div>
                </div>

                <div className={spacing.field}>
                  <label className={sectionLabel}>{t("characters.soulReview.tuningLabel")}</label>
                  <div className={spacing.item}>
                    {renderCollapsible(
                      "affect",
                      Brain,
                      t("characters.soulEditor.baselineAffect"),
                      affectChanges,
                      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        {(Object.keys(AFFECT_LABELS) as AffectKey[]).map((k) =>
                          renderSlider(
                            k,
                            AFFECT_LABELS[k],
                            working.soul.baselineAffect[k],
                            (next) => updateAffect(k, next),
                            baseline.soul.baselineAffect[k],
                          ),
                        )}
                      </div>,
                      "border-info/30 bg-info/10 text-info",
                    )}
                    {renderCollapsible(
                      "regulation",
                      SlidersHorizontal,
                      t("characters.soulEditor.regulationStyle"),
                      regulationChanges,
                      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        {(Object.keys(REGULATION_LABELS) as RegulationKey[]).map((k) =>
                          renderSlider(
                            k,
                            REGULATION_LABELS[k],
                            working.soul.regulationStyle[k],
                            (next) => updateRegulation(k, next),
                            baseline.soul.regulationStyle[k],
                          ),
                        )}
                      </div>,
                      "border-warning/30 bg-warning/10 text-warning",
                    )}
                    {renderCollapsible(
                      "relationship",
                      Shield,
                      t("characters.soulEditor.relationshipDefaults"),
                      relationshipChanges,
                      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        {(Object.keys(RELATIONSHIP_LABELS) as RelationshipKey[]).map((k) =>
                          renderSlider(
                            k,
                            RELATIONSHIP_LABELS[k],
                            working.relationshipDefaults[k],
                            (next) => updateRelationship(k, next),
                            baseline.relationshipDefaults[k],
                          ),
                        )}
                      </div>,
                      "border-secondary/30 bg-secondary/10 text-secondary",
                    )}
                  </div>
                </div>
              </div>
            </div>

            <AnimatePresence>
              {directionOpen && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.18 }}
                  className="overflow-hidden border-t border-fg/10 bg-surface-el/30"
                >
                  <div className="px-3 py-2">
                    <div className="mb-1 flex items-center justify-between px-0.5">
                      <label className="text-[10px] font-semibold uppercase tracking-[0.18em] text-fg/50">
                        {t("characters.soulReview.direction")}
                      </label>
                      <span className="text-[10px] font-mono text-fg/35">
                        {t("characters.soulReview.directionApplyHint")}
                      </span>
                    </div>
                    <textarea
                      value={direction}
                      onChange={(e) => onDirectionChange(e.target.value)}
                      rows={3}
                      autoFocus
                      placeholder='e.g. "Lean tsundere — guarded outside, soft once trusted. Less anxious."'
                      className={cn(
                        "w-full resize-none border border-fg/10 bg-surface-el/40 px-2.5 py-2 text-[12.5px] leading-relaxed text-fg outline-none",
                        radius.md,
                        interactive.transition.default,
                        "focus:border-fg/25",
                      )}
                    />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            <div className="flex items-center gap-2 border-t border-fg/10 bg-surface/95 px-3 py-2.5">
              <button
                type="button"
                onClick={() => setDirectionOpen((v) => !v)}
                className={cn(
                  "inline-flex items-center gap-1.5 border px-2.5 py-2 text-[12px] font-medium",
                  radius.md,
                  interactive.transition.fast,
                  direction.trim()
                    ? "border-info/30 bg-info/10 text-info"
                    : directionOpen
                      ? "border-fg/20 bg-fg/10 text-fg"
                      : "border-fg/10 bg-fg/5 text-fg/65 hover:bg-fg/10",
                )}
                title={t("characters.soulReview.directionTooltip")}
              >
                <Compass className="h-3.5 w-3.5" />
                <span>{t("characters.soulReview.direction")}</span>
                {direction.trim() && <span className="h-1.5 w-1.5 rounded-full bg-info" />}
              </button>
              <button
                type="button"
                onClick={onRegenerate}
                disabled={regenerating}
                className={cn(
                  "inline-flex items-center gap-1.5 border border-fg/10 bg-fg/5 px-2.5 py-2 text-[12px] font-medium text-fg/70 hover:bg-fg/10 disabled:opacity-60",
                  radius.md,
                  interactive.transition.fast,
                )}
              >
                {regenerating ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Sparkles className="h-3.5 w-3.5" />
                )}
                {t("characters.soulReview.regenerate")}
              </button>
              <div className="flex-1" />
              <button
                type="button"
                onClick={onCancel}
                className={cn(
                  "border border-fg/10 bg-fg/5 px-2.5 py-2 text-[12px] font-medium text-fg/70 hover:bg-fg/10",
                  radius.md,
                  interactive.transition.fast,
                )}
              >
                {t("characters.soulReview.discard")}
              </button>
              <button
                type="button"
                onClick={() => onApply(working)}
                className={cn(
                  "inline-flex items-center gap-1.5 border border-accent/30 bg-accent/15 px-3.5 py-2 text-[12px] font-semibold text-accent hover:bg-accent/25",
                  radius.md,
                  interactive.transition.fast,
                )}
              >
                <Check className="h-3.5 w-3.5" />
                {t("characters.soulReview.apply")}
              </button>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
