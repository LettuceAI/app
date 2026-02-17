import { Loader2 } from "lucide-react";

import type {
  SpeechPatterns,
  TimeBehaviors,
  BaselineEmotions,
} from "../../../../core/engine/types";
import type { EngineCharacterStep } from "../hooks/engineCharacterReducer";

type Props = {
  name: string;
  era: string;
  role: string;
  setting: string;
  coreIdentity: string;
  backstory: string;
  personalityTraits: string[];
  speechPatterns: SpeechPatterns;
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
  saving: boolean;
  error: string | null;
  onSave: () => void;
  onEdit: (step: EngineCharacterStep) => void;
};

function Section({
  title,
  step,
  onEdit,
  children,
}: {
  title: string;
  step: EngineCharacterStep;
  onEdit: (step: EngineCharacterStep) => void;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/5 p-3">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">{title}</h3>
        <button
          onClick={() => onEdit(step)}
          className="text-[11px] font-medium text-emerald-400 hover:text-emerald-300"
        >
          Edit
        </button>
      </div>
      <div className="space-y-1">{children}</div>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string | undefined }) {
  return (
    <div className="flex gap-2 text-xs">
      <span className="w-28 shrink-0 text-white/40">{label}</span>
      <span className={value ? "text-white/80" : "text-white/20 italic"}>
        {value || "Not set"}
      </span>
    </div>
  );
}

function TagList({ label, value }: { label: string; value: string[] }) {
  return (
    <div className="flex gap-2 text-xs">
      <span className="w-28 shrink-0 text-white/40">{label}</span>
      {value.length > 0 ? (
        <div className="flex flex-wrap gap-1">
          {value.map((tag, i) => (
            <span
              key={i}
              className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-white/70"
            >
              {tag}
            </span>
          ))}
        </div>
      ) : (
        <span className="text-white/20 italic">Not set</span>
      )}
    </div>
  );
}

export function CharacterReviewStep({
  name,
  era,
  role,
  setting,
  coreIdentity,
  backstory,
  personalityTraits,
  speechPatterns,
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
  saving,
  error,
  onSave,
  onEdit,
}: Props) {
  const hasEmotions = Object.values(baselineEmotions).some((v) => v !== undefined && v > 0);
  const hasTimeBehaviors = Object.values(timeBehaviors).some((v) => !!v);
  const hasSpeechPatterns = Object.keys(speechPatterns).length > 0;

  return (
    <div className="space-y-4 px-4 py-6">
      <div>
        <h2 className="text-lg font-semibold text-white">Review</h2>
        <p className="mt-1 text-sm text-white/50">
          Review your character before creating.
        </p>
      </div>

      <Section title="Identity" step="identity" onEdit={onEdit}>
        <Field label="Name" value={name} />
        <Field label="Era" value={era} />
        <Field label="Role" value={role} />
        <Field label="Setting" value={setting} />
        <Field label="Core Identity" value={coreIdentity} />
        <Field label="Backstory" value={backstory} />
      </Section>

      <Section title="Personality" step="personality" onEdit={onEdit}>
        <TagList label="Traits" value={personalityTraits} />
        {hasSpeechPatterns && (
          <>
            <Field label="Formality" value={speechPatterns.formality} />
            <Field label="Verbosity" value={speechPatterns.verbosity} />
            <Field label="Dialect" value={speechPatterns.dialect} />
            <TagList label="Catchphrases" value={speechPatterns.catchphrases || []} />
          </>
        )}
      </Section>

      <Section title="World & Behavior" step="world" onEdit={onEdit}>
        <TagList label="Domains" value={knowledgeDomains} />
        <TagList label="Boundaries" value={knowledgeBoundaries} />
        <TagList label="Research Seeds" value={researchSeeds} />
        <Field label="Research" value={researchEnabled ? "Enabled" : "Disabled"} />
        <Field label="Physical" value={physicalDescription} />
        <TagList label="Habits" value={physicalHabits} />
        <TagList label="Idle" value={idleBehaviors} />
        {hasTimeBehaviors && <Field label="Time Behaviors" value="Configured" />}
        {hasEmotions && <Field label="Emotions" value="Configured" />}
        {(backend || model) && (
          <>
            <Field label="Backend" value={backend} />
            <Field label="Model" value={model} />
            <Field label="Temperature" value={String(temperature)} />
          </>
        )}
      </Section>

      {error && <p className="text-xs font-medium text-rose-300">{error}</p>}

      <button
        onClick={onSave}
        disabled={saving || !name.trim()}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {saving ? (
          <span className="flex items-center justify-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin" />
            Creating...
          </span>
        ) : (
          "Create Character"
        )}
      </button>
    </div>
  );
}
