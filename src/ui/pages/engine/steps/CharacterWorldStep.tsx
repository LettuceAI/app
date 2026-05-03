import type { TimeBehaviors, BaselineEmotions } from "../../../../core/engine/types";
import { TagInput } from "../components/TagInput";
import { CollapsibleSection } from "../components/CollapsibleSection";
import { Switch } from "../../../components/Switch";
import { useI18n } from "../../../../core/i18n/context";

type Props = {
  knowledgeDomains: string[];
  knowledgeBoundaries: string[];
  researchSeeds: string[];
  researchEnabled: boolean;
  physicalDescription: string;
  physicalHabits: string[];
  idleBehaviors: string[];
  timeBehaviors: TimeBehaviors;
  baselineEmotions: BaselineEmotions;
  backend: string;
  model: string;
  temperature: number;
  onFieldChange: (field: string, value: unknown) => void;
  onTimeBehaviorChange: (field: string, value: string) => void;
  onEmotionChange: (field: string, value: number) => void;
  onNext: () => void;
};

const EMOTION_FIELD_DEFS: { field: keyof BaselineEmotions; labelKey: string; color: string }[] = [
  { field: "joy", labelKey: "joy", color: "bg-yellow-400" },
  { field: "trust", labelKey: "trust", color: "bg-green-400" },
  { field: "fear", labelKey: "fear", color: "bg-purple-400" },
  { field: "surprise", labelKey: "surprise", color: "bg-cyan-400" },
  { field: "sadness", labelKey: "sadness", color: "bg-blue-400" },
  { field: "disgust", labelKey: "disgust", color: "bg-lime-400" },
  { field: "anger", labelKey: "anger", color: "bg-rose-400" },
  { field: "anticipation", labelKey: "anticipation", color: "bg-orange-400" },
];

const TIME_FIELD_DEFS: { field: keyof TimeBehaviors; labelKey: string }[] = [
  { field: "early_morning", labelKey: "earlyMorning" },
  { field: "morning", labelKey: "morning" },
  { field: "afternoon", labelKey: "afternoon" },
  { field: "evening", labelKey: "evening" },
  { field: "night", labelKey: "night" },
];

export function CharacterWorldStep({
  knowledgeDomains,
  knowledgeBoundaries,
  researchSeeds,
  researchEnabled,
  physicalDescription,
  physicalHabits,
  idleBehaviors,
  timeBehaviors,
  baselineEmotions,
  backend,
  model,
  temperature,
  onFieldChange,
  onTimeBehaviorChange,
  onEmotionChange,
  onNext,
}: Props) {
  const { t: tRoot, scope } = useI18n();
  const t = scope("engine.characterCreate.steps.world");
  return (
    <div className="space-y-4 px-4 py-6">
      <h2 className="text-lg font-semibold text-white">{t("title")}</h2>

      {/* Knowledge */}
      <TagInput
        label={t("knowledgeDomains")}
        value={knowledgeDomains}
        onChange={(v) => onFieldChange("knowledgeDomains", v)}
        placeholder={t("knowledgeDomainsPlaceholder")}
      />
      <TagInput
        label={t("knowledgeBoundaries")}
        value={knowledgeBoundaries}
        onChange={(v) => onFieldChange("knowledgeBoundaries", v)}
        placeholder={t("knowledgeBoundariesPlaceholder")}
      />
      <TagInput
        label={t("researchSeeds")}
        value={researchSeeds}
        onChange={(v) => onFieldChange("researchSeeds", v)}
        placeholder={t("researchSeedsPlaceholder")}
      />

      <div className="flex items-center justify-between rounded-xl border border-white/10 bg-white/5 px-4 py-3">
        <div>
          <p className="text-sm font-medium text-white/80">{t("researchEnabled")}</p>
          <p className="text-[11px] text-white/45">{t("researchEnabledDesc")}</p>
        </div>
        <Switch
          checked={researchEnabled}
          onChange={(next) => onFieldChange("researchEnabled", next)}
        />
      </div>

      {/* Physical */}
      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">
          {t("physicalDescription")}
        </label>
        <textarea
          value={physicalDescription}
          onChange={(e) => onFieldChange("physicalDescription", e.target.value)}
          placeholder={t("physicalDescPlaceholder")}
          rows={2}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>
      <TagInput
        label={t("physicalHabits")}
        value={physicalHabits}
        onChange={(v) => onFieldChange("physicalHabits", v)}
        placeholder={t("physicalHabitsPlaceholder")}
      />
      <TagInput
        label={t("idleBehaviors")}
        value={idleBehaviors}
        onChange={(v) => onFieldChange("idleBehaviors", v)}
        placeholder={t("idleBehaviorsPlaceholder")}
      />

      {/* Time Behaviors */}
      <CollapsibleSection title={t("timeBehaviors")}>
        {TIME_FIELD_DEFS.map(({ field, labelKey }) => {
          const label = t(labelKey);
          return (
            <div key={field}>
              <label className="mb-1 block text-[11px] font-medium text-white/70">{label}</label>
              <textarea
                value={timeBehaviors[field] || ""}
                onChange={(e) => onTimeBehaviorChange(field, e.target.value)}
                placeholder={t("timePlaceholder", { period: label.toLowerCase() })}
                rows={2}
                className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
              />
            </div>
          );
        })}
      </CollapsibleSection>

      {/* Emotions */}
      <CollapsibleSection title={t("baselineEmotions")}>
        <p className="text-[11px] text-white/40 -mt-1">
          {t("emotionDesc")}
        </p>
        {EMOTION_FIELD_DEFS.map(({ field, labelKey, color }) => (
          <div key={field} className="flex items-center gap-3">
            <div className="flex items-center gap-2 w-28">
              <span className={`h-2 w-2 rounded-full ${color}`} />
              <span className="text-xs text-white/60">{t(labelKey)}</span>
            </div>
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={baselineEmotions[field] ?? 0}
              onChange={(e) => onEmotionChange(field, parseFloat(e.target.value))}
              className="flex-1 accent-emerald-400"
            />
            <span className="w-8 text-right text-[11px] text-white/50">
              {(baselineEmotions[field] ?? 0).toFixed(2)}
            </span>
          </div>
        ))}
      </CollapsibleSection>

      {/* Engine Overrides */}
      <CollapsibleSection title={t("engineOverrides")}>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("backend")}</label>
          <input
            type="text"
            value={backend}
            onChange={(e) => onFieldChange("backend", e.target.value)}
            placeholder={t("leaveEmpty")}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("model")}</label>
          <input
            type="text"
            value={model}
            onChange={(e) => onFieldChange("model", e.target.value)}
            placeholder={t("leaveEmpty")}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("temperature")}</label>
          <input
            type="number"
            step="0.1"
            min="0"
            max="2"
            value={temperature}
            onChange={(e) =>
              onFieldChange(
                "temperature",
                Math.min(2, Math.max(0, parseFloat(e.target.value) || 0.9)),
              )
            }
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
      </CollapsibleSection>

      <button
        onClick={onNext}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30"
      >
        {tRoot("common.buttons.continue")}
      </button>
    </div>
  );
}
