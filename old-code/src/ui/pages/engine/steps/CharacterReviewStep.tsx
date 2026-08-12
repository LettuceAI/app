import { Loader2 } from "lucide-react";

import { useI18n } from "../../../../core/i18n/context";
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
  editLabel,
  onEdit,
  children,
}: {
  title: string;
  step: EngineCharacterStep;
  editLabel: string;
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
          {editLabel}
        </button>
      </div>
      <div className="space-y-1">{children}</div>
    </div>
  );
}

function Field({
  label,
  value,
  emptyLabel,
}: {
  label: string;
  value: string | undefined;
  emptyLabel: string;
}) {
  return (
    <div className="flex gap-2 text-xs">
      <span className="w-28 shrink-0 text-white/40">{label}</span>
      <span className={value ? "text-white/80" : "text-white/20 italic"}>
        {value || emptyLabel}
      </span>
    </div>
  );
}

function TagList({
  label,
  value,
  emptyLabel,
}: {
  label: string;
  value: string[];
  emptyLabel: string;
}) {
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
        <span className="text-white/20 italic">{emptyLabel}</span>
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
  const { scope } = useI18n();
  const t = scope("engine.characterCreate.steps.review");
  const hasEmotions = Object.values(baselineEmotions).some((v) => v !== undefined && v > 0);
  const hasTimeBehaviors = Object.values(timeBehaviors).some((v) => !!v);
  const hasSpeechPatterns = Object.keys(speechPatterns).length > 0;

  return (
    <div className="space-y-4 px-4 py-6">
      <div>
        <h2 className="text-lg font-semibold text-white">{t("title")}</h2>
        <p className="mt-1 text-sm text-white/50">{t("subtitle")}</p>
      </div>

      <Section title={t("identitySection")} step="identity" editLabel={t("edit")} onEdit={onEdit}>
        <Field label={t("nameLabel")} value={name} emptyLabel={t("notSet")} />
        <Field label={t("eraLabel")} value={era} emptyLabel={t("notSet")} />
        <Field label={t("roleLabel")} value={role} emptyLabel={t("notSet")} />
        <Field label={t("settingLabel")} value={setting} emptyLabel={t("notSet")} />
        <Field label={t("coreIdentityLabel")} value={coreIdentity} emptyLabel={t("notSet")} />
        <Field label={t("backstoryLabel")} value={backstory} emptyLabel={t("notSet")} />
      </Section>

      <Section
        title={t("personalitySection")}
        step="personality"
        editLabel={t("edit")}
        onEdit={onEdit}
      >
        <TagList label={t("traitsLabel")} value={personalityTraits} emptyLabel={t("notSet")} />
        {hasSpeechPatterns && (
          <>
            <Field
              label={t("formalityLabel")}
              value={speechPatterns.formality}
              emptyLabel={t("notSet")}
            />
            <Field
              label={t("verbosityLabel")}
              value={speechPatterns.verbosity}
              emptyLabel={t("notSet")}
            />
            <Field
              label={t("dialectLabel")}
              value={speechPatterns.dialect}
              emptyLabel={t("notSet")}
            />
            <TagList
              label={t("catchphrasesLabel")}
              value={speechPatterns.catchphrases || []}
              emptyLabel={t("notSet")}
            />
          </>
        )}
      </Section>

      <Section
        title={t("worldSection")}
        step="world"
        editLabel={t("edit")}
        onEdit={onEdit}
      >
        <TagList label={t("domainsLabel")} value={knowledgeDomains} emptyLabel={t("notSet")} />
        <TagList
          label={t("boundariesLabel")}
          value={knowledgeBoundaries}
          emptyLabel={t("notSet")}
        />
        <TagList
          label={t("researchSeedsLabel")}
          value={researchSeeds}
          emptyLabel={t("notSet")}
        />
        <Field
          label={t("researchLabel")}
          value={researchEnabled ? t("enabled") : t("disabled")}
          emptyLabel={t("notSet")}
        />
        <Field label={t("physicalLabel")} value={physicalDescription} emptyLabel={t("notSet")} />
        <TagList label={t("habitsLabel")} value={physicalHabits} emptyLabel={t("notSet")} />
        <TagList label={t("idleLabel")} value={idleBehaviors} emptyLabel={t("notSet")} />
        {hasTimeBehaviors && (
          <Field
            label={t("timeBehaviorsLabel")}
            value={t("configured")}
            emptyLabel={t("notSet")}
          />
        )}
        {hasEmotions && (
          <Field label={t("emotionsLabel")} value={t("configured")} emptyLabel={t("notSet")} />
        )}
        {(backend || model) && (
          <>
            <Field label={t("backendLabel")} value={backend} emptyLabel={t("notSet")} />
            <Field label={t("modelLabel")} value={model} emptyLabel={t("notSet")} />
            <Field
              label={t("temperatureLabel")}
              value={String(temperature)}
              emptyLabel={t("notSet")}
            />
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
            {t("creating")}
          </span>
        ) : (
          t("createCharacter")
        )}
      </button>
    </div>
  );
}
