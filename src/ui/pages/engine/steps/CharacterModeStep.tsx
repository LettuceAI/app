import { Sparkles, PenLine, Loader2 } from "lucide-react";

type Props = {
  boostName: string;
  boostSeed: string;
  boostEra: string;
  boosting: boolean;
  boostError: string | null;
  onBoostFieldChange: (field: "boostName" | "boostSeed" | "boostEra", value: string) => void;
  onBoost: () => void;
  onManual: () => void;
};

export function CharacterModeStep({
  boostName,
  boostSeed,
  boostEra,
  boosting,
  boostError,
  onBoostFieldChange,
  onBoost,
  onManual,
}: Props) {
  return (
    <div className="space-y-4 px-4 py-6">
      <div>
        <h2 className="text-lg font-semibold text-white">Create Character</h2>
        <p className="mt-1 text-sm text-white/50">
          Generate a character with AI or build one from scratch.
        </p>
      </div>

      {/* AI Boost */}
      <div className="rounded-xl border border-indigo-400/20 bg-indigo-500/10 p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Sparkles className="h-4 w-4 text-indigo-300" />
          <span className="text-sm font-medium text-indigo-200">AI Boost</span>
        </div>
        <p className="text-xs text-white/50">
          Describe your character idea and the AI will generate a full character definition.
        </p>

        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">
            Name (optional)
          </label>
          <input
            type="text"
            value={boostName}
            onChange={(e) => onBoostFieldChange("boostName", e.target.value)}
            placeholder="e.g. Marcus Cole"
            disabled={boosting}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
          />
        </div>

        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">
            Seed Description *
          </label>
          <textarea
            value={boostSeed}
            onChange={(e) => onBoostFieldChange("boostSeed", e.target.value)}
            placeholder="e.g. jazz pianist in 1950s Harlem, philosophical, loves late-night conversations"
            disabled={boosting}
            rows={3}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none disabled:opacity-50"
          />
        </div>

        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">
            Era (optional)
          </label>
          <input
            type="text"
            value={boostEra}
            onChange={(e) => onBoostFieldChange("boostEra", e.target.value)}
            placeholder="e.g. 1950s, modern, Victorian"
            disabled={boosting}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none disabled:opacity-50"
          />
        </div>

        {boostError && <p className="text-xs font-medium text-rose-300">{boostError}</p>}

        <button
          onClick={onBoost}
          disabled={boosting || !boostSeed.trim()}
          className="w-full rounded-lg border border-indigo-400/40 bg-indigo-500/20 px-4 py-3 text-sm font-semibold text-indigo-100 transition hover:border-indigo-400/60 hover:bg-indigo-500/30 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {boosting ? (
            <span className="flex items-center justify-center gap-2">
              <Loader2 className="h-4 w-4 animate-spin" />
              Generating...
            </span>
          ) : (
            <span className="flex items-center justify-center gap-2">
              <Sparkles className="h-4 w-4" />
              Generate Character
            </span>
          )}
        </button>
      </div>

      {/* Manual */}
      <div className="relative flex items-center gap-3 py-1">
        <div className="flex-1 border-t border-white/10" />
        <span className="text-[11px] text-white/30">or</span>
        <div className="flex-1 border-t border-white/10" />
      </div>

      <button
        onClick={onManual}
        disabled={boosting}
        className="flex w-full items-center justify-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm font-medium text-white/70 transition hover:border-white/20 hover:bg-white/10 disabled:opacity-50"
      >
        <PenLine className="h-4 w-4" />
        Start from Scratch
      </button>
    </div>
  );
}
