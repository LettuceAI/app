import type { SpeechPatterns } from "../../../../core/engine/types";
import { TagInput } from "../components/TagInput";
import { CollapsibleSection } from "../components/CollapsibleSection";

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
  return (
    <div className="space-y-4 px-4 py-6">
      <h2 className="text-lg font-semibold text-white">Personality</h2>

      <TagInput
        label="Personality Traits"
        value={personalityTraits}
        onChange={(v) => onFieldChange("personalityTraits", v)}
        placeholder="e.g. witty, compassionate, stubborn"
      />

      <CollapsibleSection title="Speech Patterns">
        <SegmentedControl
          label="Formality"
          value={speechPatterns.formality}
          options={[
            { value: "formal", label: "Formal" },
            { value: "casual", label: "Casual" },
            { value: "texting", label: "Texting" },
          ]}
          onChange={(v) => onSpeechPatternChange("formality", v)}
        />
        <SegmentedControl
          label="Verbosity"
          value={speechPatterns.verbosity}
          options={[
            { value: "terse", label: "Terse" },
            { value: "medium", label: "Medium" },
            { value: "verbose", label: "Verbose" },
          ]}
          onChange={(v) => onSpeechPatternChange("verbosity", v)}
        />
        <SegmentedControl
          label="Text Style"
          value={speechPatterns.text_style}
          options={[
            { value: "formal", label: "Formal" },
            { value: "casual", label: "Casual" },
            { value: "texting", label: "Texting" },
          ]}
          onChange={(v) => onSpeechPatternChange("text_style", v)}
        />
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Dialect</label>
          <input
            type="text"
            value={speechPatterns.dialect || ""}
            onChange={(e) => onSpeechPatternChange("dialect", e.target.value)}
            placeholder="e.g. Southern American, British RP"
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <TagInput
          label="Catchphrases"
          value={speechPatterns.catchphrases || []}
          onChange={(v) => onSpeechPatternChange("catchphrases", v)}
          placeholder="e.g. Well I'll be..."
        />
        <TagInput
          label="Vocabulary Preferences"
          value={speechPatterns.vocabulary_preferences || []}
          onChange={(v) => onSpeechPatternChange("vocabulary_preferences", v)}
          placeholder="Words they favor"
        />
        <TagInput
          label="Vocabulary Avoidances"
          value={speechPatterns.vocabulary_avoidances || []}
          onChange={(v) => onSpeechPatternChange("vocabulary_avoidances", v)}
          placeholder="Words they avoid"
        />
        <TagInput
          label="Filler Words"
          value={speechPatterns.filler_words || []}
          onChange={(v) => onSpeechPatternChange("filler_words", v)}
          placeholder="e.g. um, like, you know"
        />
        <TagInput
          label="Example Quotes"
          value={speechPatterns.example_quotes || []}
          onChange={(v) => onSpeechPatternChange("example_quotes", v)}
          placeholder="3-5 example lines of dialogue"
        />
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
