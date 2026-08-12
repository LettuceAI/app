import type { SpeechPatterns } from "../../../../core/engine/types";
import { TagInput } from "../components/TagInput";
import { CollapsibleSection } from "../components/CollapsibleSection";
import { useI18n } from "../../../../core/i18n/context";

type Props = {
  personalityTraits: string[];
  speechPatterns: SpeechPatterns;
  onFieldChange: (field: string, value: unknown) => void;
  onSpeechPatternChange: (field: string, value: unknown) => void;
  onNext: () => void;
};

function SegmentedControl({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string | undefined;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
}) {
  return (
    <div>
      <label className="mb-1 block text-[11px] font-medium text-white/70">{label}</label>
      <div className="flex rounded-lg border border-white/10 bg-black/20 overflow-hidden">
        {options.map((opt) => (
          <button
            key={opt.value}
            onClick={() => onChange(opt.value)}
            className={`flex-1 px-2 py-1.5 text-[11px] font-medium transition ${
              value === opt.value
                ? "bg-white/15 text-white"
                : "text-white/40 hover:text-white/60"
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function CharacterPersonalityStep({
  personalityTraits,
  speechPatterns,
  onFieldChange,
  onSpeechPatternChange,
  onNext,
}: Props) {
  const { t } = useI18n();
  return (
    <div className="space-y-4 px-4 py-6">
      <h2 className="text-lg font-semibold text-white">{t("engine.characterCreate.steps.personality.title")}</h2>

      <TagInput
        label={t("engine.characterCreate.steps.personality.traits")}
        value={personalityTraits}
        onChange={(v) => onFieldChange("personalityTraits", v)}
        placeholder={t("engine.characterCreate.steps.personality.traitsPlaceholder")}
      />

      <CollapsibleSection title={t("engine.characterCreate.steps.personality.speechPatterns")}>
        <SegmentedControl
          label={t("engine.characterCreate.steps.personality.formality")}
          value={speechPatterns.formality}
          options={[
            { value: "formal", label: t("engine.characterCreate.steps.personality.formal") },
            { value: "casual", label: t("engine.characterCreate.steps.personality.casual") },
            { value: "texting", label: t("engine.characterCreate.steps.personality.texting") },
          ]}
          onChange={(v) => onSpeechPatternChange("formality", v)}
        />
        <SegmentedControl
          label={t("engine.characterCreate.steps.personality.verbosity")}
          value={speechPatterns.verbosity}
          options={[
            { value: "terse", label: t("engine.characterCreate.steps.personality.terse") },
            { value: "medium", label: t("engine.characterCreate.steps.personality.medium") },
            { value: "verbose", label: t("engine.characterCreate.steps.personality.verbose") },
          ]}
          onChange={(v) => onSpeechPatternChange("verbosity", v)}
        />
        <SegmentedControl
          label={t("engine.characterCreate.steps.personality.textStyle")}
          value={speechPatterns.text_style}
          options={[
            { value: "formal", label: t("engine.characterCreate.steps.personality.formal") },
            { value: "casual", label: t("engine.characterCreate.steps.personality.casual") },
            { value: "texting", label: t("engine.characterCreate.steps.personality.texting") },
          ]}
          onChange={(v) => onSpeechPatternChange("text_style", v)}
        />
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.personality.dialect")}</label>
          <input
            type="text"
            value={speechPatterns.dialect || ""}
            onChange={(e) => onSpeechPatternChange("dialect", e.target.value)}
            placeholder={t("engine.characterCreate.steps.personality.dialectPlaceholder")}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <TagInput
          label={t("engine.characterCreate.steps.personality.catchphrases")}
          value={speechPatterns.catchphrases || []}
          onChange={(v) => onSpeechPatternChange("catchphrases", v)}
          placeholder={t("engine.characterCreate.steps.personality.catchphrasesPlaceholder")}
        />
        <TagInput
          label={t("engine.characterCreate.steps.personality.vocabPreferences")}
          value={speechPatterns.vocabulary_preferences || []}
          onChange={(v) => onSpeechPatternChange("vocabulary_preferences", v)}
          placeholder={t("engine.characterCreate.steps.personality.vocabPreferencesPlaceholder")}
        />
        <TagInput
          label={t("engine.characterCreate.steps.personality.vocabAvoidances")}
          value={speechPatterns.vocabulary_avoidances || []}
          onChange={(v) => onSpeechPatternChange("vocabulary_avoidances", v)}
          placeholder={t("engine.characterCreate.steps.personality.vocabAvoidancesPlaceholder")}
        />
        <TagInput
          label={t("engine.characterCreate.steps.personality.fillerWords")}
          value={speechPatterns.filler_words || []}
          onChange={(v) => onSpeechPatternChange("filler_words", v)}
          placeholder={t("engine.characterCreate.steps.personality.fillerWordsPlaceholder")}
        />
        <TagInput
          label={t("engine.characterCreate.steps.personality.exampleQuotes")}
          value={speechPatterns.example_quotes || []}
          onChange={(v) => onSpeechPatternChange("example_quotes", v)}
          placeholder={t("engine.characterCreate.steps.personality.exampleQuotesPlaceholder")}
        />
      </CollapsibleSection>

      <button
        onClick={onNext}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30"
      >
        {t("common.buttons.continue")}
      </button>
    </div>
  );
}
