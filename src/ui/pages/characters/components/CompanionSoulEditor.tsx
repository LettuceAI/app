import { Brain, Heart, Shield, SlidersHorizontal } from "lucide-react";
import type { CompanionConfig } from "../../../../core/storage/schemas";
import { cn, radius, spacing, typography } from "../../../design-tokens";
import { normalizeCompanionConfig } from "../utils/companionDefaults";

type SoulTextKey =
  | "essence"
  | "voice"
  | "relationalStyle"
  | "vulnerabilities"
  | "habits"
  | "boundaries";

type AffectKey = keyof CompanionConfig["soul"]["baselineAffect"];
type RegulationKey = keyof CompanionConfig["soul"]["regulationStyle"];
type RelationshipKey = keyof CompanionConfig["relationshipDefaults"];

interface CompanionSoulEditorProps {
  companion: CompanionConfig | null | undefined;
  onChange: (next: CompanionConfig) => void;
  disabled?: boolean;
}

const SOUL_TEXT_FIELDS: Array<{
  key: SoulTextKey;
  label: string;
  placeholder: string;
  rows: number;
}> = [
  {
    key: "essence",
    label: "Essence",
    rows: 3,
    placeholder: "Who they are underneath the card definition. What stays true across moods?",
  },
  {
    key: "voice",
    label: "Inner Voice",
    rows: 3,
    placeholder: "How their companion presence feels in direct conversation.",
  },
  {
    key: "relationalStyle",
    label: "Relational Style",
    rows: 3,
    placeholder: "How they attach, trust, comfort, tease, withdraw, or reconnect.",
  },
  {
    key: "vulnerabilities",
    label: "Vulnerabilities",
    rows: 2,
    placeholder: "Soft spots, insecurities, old wounds, or needs they rarely say directly.",
  },
  {
    key: "habits",
    label: "Habits",
    rows: 2,
    placeholder: "Small recurring behaviors, rituals, tells, and conversational habits.",
  },
  {
    key: "boundaries",
    label: "Boundaries",
    rows: 2,
    placeholder: "Emotional boundaries, refusal lines, pace, and relationship limits.",
  },
];

const AFFECT_SLIDERS: Array<{ key: AffectKey; label: string }> = [
  { key: "warmth", label: "Warmth" },
  { key: "trust", label: "Trust" },
  { key: "calm", label: "Calm" },
  { key: "vulnerability", label: "Vulnerability" },
  { key: "longing", label: "Longing" },
  { key: "hurt", label: "Hurt" },
  { key: "tension", label: "Tension" },
  { key: "irritation", label: "Irritation" },
  { key: "affectionIntensity", label: "Affection" },
  { key: "reassuranceNeed", label: "Reassurance Need" },
];

const REGULATION_SLIDERS: Array<{ key: RegulationKey; label: string }> = [
  { key: "suppression", label: "Suppression" },
  { key: "volatility", label: "Volatility" },
  { key: "recoverySpeed", label: "Recovery Speed" },
  { key: "conflictAvoidance", label: "Conflict Avoidance" },
  { key: "reassuranceSeeking", label: "Reassurance Seeking" },
  { key: "protestBehavior", label: "Protest Behavior" },
  { key: "emotionalTransparency", label: "Transparency" },
  { key: "attachmentActivation", label: "Attachment Activation" },
  { key: "pride", label: "Pride" },
];

const RELATIONSHIP_SLIDERS: Array<{ key: RelationshipKey; label: string }> = [
  { key: "closeness", label: "Starting Closeness" },
  { key: "trust", label: "Starting Trust" },
  { key: "affection", label: "Starting Affection" },
  { key: "tension", label: "Starting Tension" },
];

function pct(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function normalizeCompanion(companion: CompanionConfig | null | undefined): CompanionConfig {
  return normalizeCompanionConfig(companion);
}

export function CompanionSoulEditor({
  companion,
  onChange,
  disabled = false,
}: CompanionSoulEditorProps) {
  const value = normalizeCompanion(companion);

  const updateSoulText = (key: SoulTextKey, nextValue: string) => {
    onChange({
      ...value,
      soul: {
        ...value.soul,
        [key]: nextValue,
      },
    });
  };

  const updateAffect = (key: AffectKey, nextValue: number) => {
    onChange({
      ...value,
      soul: {
        ...value.soul,
        baselineAffect: {
          ...value.soul.baselineAffect,
          [key]: nextValue,
        },
      },
    });
  };

  const updateRegulation = (key: RegulationKey, nextValue: number) => {
    onChange({
      ...value,
      soul: {
        ...value.soul,
        regulationStyle: {
          ...value.soul.regulationStyle,
          [key]: nextValue,
        },
      },
    });
  };

  const updateRelationship = (key: RelationshipKey, nextValue: number) => {
    onChange({
      ...value,
      relationshipDefaults: {
        ...value.relationshipDefaults,
        [key]: nextValue,
      },
    });
  };

  const renderSlider = (
    key: string,
    label: string,
    sliderValue: number,
    onSliderChange: (next: number) => void,
  ) => (
    <label key={key} className="block rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5">
      <div className="mb-2 flex items-center justify-between gap-3">
        <span className="text-xs font-medium text-fg/70">{label}</span>
        <span className="rounded-md border border-fg/10 bg-fg/10 px-1.5 py-0.5 text-[10px] text-fg/50">
          {pct(sliderValue)}
        </span>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        step={1}
        disabled={disabled}
        value={Math.round(sliderValue * 100)}
        onChange={(event) => onSliderChange(Number(event.target.value) / 100)}
        className="w-full accent-accent disabled:opacity-50"
      />
    </label>
  );

  return (
    <section
      className={cn(
        "overflow-hidden rounded-2xl border border-accent/15 bg-accent/5",
        disabled && "opacity-60",
      )}
    >
      <div className="border-b border-accent/10 px-4 py-3">
        <div className="flex items-start gap-3">
          <div className="rounded-xl border border-accent/25 bg-accent/10 p-2">
            <Heart className="h-4 w-4 text-accent" />
          </div>
          <div>
            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>
              Companion Soul
            </h3>
            <p className={cn(typography.bodySmall.size, "mt-1 text-fg/50")}>
              Persistent companion identity, emotional baseline, and regulation rules.
            </p>
          </div>
        </div>
      </div>

      <div className={cn(spacing.item, "p-4")}>
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
          {SOUL_TEXT_FIELDS.map((field) => (
            <label key={field.key} className={cn(field.key === "essence" && "lg:col-span-2")}>
              <span className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.22em] text-fg/35">
                {field.label}
              </span>
              <textarea
                value={value.soul[field.key]}
                onChange={(event) => updateSoulText(field.key, event.target.value)}
                rows={field.rows}
                disabled={disabled}
                placeholder={field.placeholder}
                className={cn(
                  "w-full resize-none border border-fg/10 bg-surface-el/25 px-3.5 py-3 text-sm leading-relaxed text-fg placeholder:text-fg/35",
                  radius.md,
                  "transition focus:border-accent/35 focus:bg-surface-el/35 focus:outline-none disabled:cursor-not-allowed",
                )}
              />
            </label>
          ))}
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <Brain className="h-4 w-4 text-info" />
              <h4 className="text-sm font-semibold text-fg">Baseline Affect</h4>
            </div>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-1">
              {AFFECT_SLIDERS.map((item) =>
                renderSlider(item.key, item.label, value.soul.baselineAffect[item.key], (next) =>
                  updateAffect(item.key, next),
                ),
              )}
            </div>
          </div>

          <div>
            <div className="mb-2 flex items-center gap-2">
              <SlidersHorizontal className="h-4 w-4 text-warning" />
              <h4 className="text-sm font-semibold text-fg">Regulation</h4>
            </div>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-1">
              {REGULATION_SLIDERS.map((item) =>
                renderSlider(
                  item.key,
                  item.label,
                  value.soul.regulationStyle[item.key],
                  (next) => updateRegulation(item.key, next),
                ),
              )}
            </div>
          </div>

          <div>
            <div className="mb-2 flex items-center gap-2">
              <Shield className="h-4 w-4 text-secondary" />
              <h4 className="text-sm font-semibold text-fg">Relationship Defaults</h4>
            </div>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-1">
              {RELATIONSHIP_SLIDERS.map((item) =>
                renderSlider(item.key, item.label, value.relationshipDefaults[item.key], (next) =>
                  updateRelationship(item.key, next),
                ),
              )}
            </div>
            <p className="mt-3 rounded-xl border border-fg/10 bg-fg/5 px-3 py-2 text-[11px] leading-relaxed text-fg/45">
              These values seed a new companion session. After that, the local emotion engine
              updates relationship state from the conversation.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
