import type { TimeBehaviors, BaselineEmotions } from "../../../../core/engine/types";
import { TagInput } from "../components/TagInput";
import { CollapsibleSection } from "../components/CollapsibleSection";

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

const EMOTION_LABELS: { field: keyof BaselineEmotions; label: string; color: string }[] = [
  { field: "joy", label: "Joy", color: "bg-yellow-400" },
  { field: "trust", label: "Trust", color: "bg-green-400" },
  { field: "fear", label: "Fear", color: "bg-purple-400" },
  { field: "surprise", label: "Surprise", color: "bg-cyan-400" },
  { field: "sadness", label: "Sadness", color: "bg-blue-400" },
  { field: "disgust", label: "Disgust", color: "bg-lime-400" },
  { field: "anger", label: "Anger", color: "bg-rose-400" },
  { field: "anticipation", label: "Anticipation", color: "bg-orange-400" },
];

const TIME_PERIODS: { field: keyof TimeBehaviors; label: string }[] = [
  { field: "early_morning", label: "Early Morning" },
  { field: "morning", label: "Morning" },
  { field: "afternoon", label: "Afternoon" },
  { field: "evening", label: "Evening" },
  { field: "night", label: "Night" },
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
  return (
    <div className="space-y-4 px-4 py-6">
      <h2 className="text-lg font-semibold text-white">World & Behavior</h2>

      {/* Knowledge */}
      <TagInput
        label="Knowledge Domains"
        value={knowledgeDomains}
        onChange={(v) => onFieldChange("knowledgeDomains", v)}
        placeholder="e.g. jazz history, music theory"
      />
      <TagInput
        label="Knowledge Boundaries"
        value={knowledgeBoundaries}
        onChange={(v) => onFieldChange("knowledgeBoundaries", v)}
        placeholder="Topics they don't know about"
      />
      <TagInput
        label="Research Seeds"
        value={researchSeeds}
        onChange={(v) => onFieldChange("researchSeeds", v)}
        placeholder="Starting topics for background research"
      />

      <div className="flex items-center justify-between rounded-xl border border-white/10 bg-white/5 px-4 py-3">
        <div>
          <p className="text-sm font-medium text-white/80">Research Enabled</p>
          <p className="text-[11px] text-white/45">Allow background knowledge gathering</p>
        </div>
        <button
          onClick={() => onFieldChange("researchEnabled", !researchEnabled)}
          className={`relative inline-flex h-6 w-11 shrink-0 rounded-full border-2 border-transparent transition-all ${
            researchEnabled ? "bg-emerald-500" : "bg-white/20"
          }`}
        >
          <span
            className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition ${
              researchEnabled ? "translate-x-5" : "translate-x-0"
            }`}
          />
        </button>
      </div>

      {/* Physical */}
      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">
          Physical Description
        </label>
        <textarea
          value={physicalDescription}
          onChange={(e) => onFieldChange("physicalDescription", e.target.value)}
          placeholder="Physical appearance and mannerisms..."
          rows={2}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>
      <TagInput
        label="Physical Habits"
        value={physicalHabits}
        onChange={(v) => onFieldChange("physicalHabits", v)}
        placeholder="e.g. taps fingers, adjusts glasses"
      />
      <TagInput
        label="Idle Behaviors"
        value={idleBehaviors}
        onChange={(v) => onFieldChange("idleBehaviors", v)}
        placeholder="What they do when not engaged"
      />

      {/* Time Behaviors */}
      <CollapsibleSection title="Time Behaviors">
        {TIME_PERIODS.map(({ field, label }) => (
          <div key={field}>
            <label className="mb-1 block text-[11px] font-medium text-white/70">{label}</label>
            <textarea
              value={timeBehaviors[field] || ""}
              onChange={(e) => onTimeBehaviorChange(field, e.target.value)}
              placeholder={`What do they do during ${label.toLowerCase()}?`}
              rows={2}
              className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
            />
          </div>
        ))}
      </CollapsibleSection>

      {/* Emotions */}
      <CollapsibleSection title="Baseline Emotions (Plutchik)">
        <p className="text-[11px] text-white/40 -mt-1">
          Set default emotional baseline (0 = none, 1 = maximum)
        </p>
        {EMOTION_LABELS.map(({ field, label, color }) => (
          <div key={field} className="flex items-center gap-3">
            <div className="flex items-center gap-2 w-28">
              <span className={`h-2 w-2 rounded-full ${color}`} />
              <span className="text-xs text-white/60">{label}</span>
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
      <CollapsibleSection title="Engine Overrides">
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Backend</label>
          <input
            type="text"
            value={backend}
            onChange={(e) => onFieldChange("backend", e.target.value)}
            placeholder="Leave empty for default"
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Model</label>
          <input
            type="text"
            value={model}
            onChange={(e) => onFieldChange("model", e.target.value)}
            placeholder="Leave empty for default"
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Temperature</label>
          <input
            type="number"
            step="0.1"
            min="0"
            max="2"
            value={temperature}
            onChange={(e) =>
              onFieldChange("temperature", Math.min(2, Math.max(0, parseFloat(e.target.value) || 0.9)))
            }
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
      </CollapsibleSection>

      <button
        onClick={onNext}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30"
      >
        Continue
      </button>
    </div>
  );
}
