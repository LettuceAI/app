type Props = {
  name: string;
  era: string;
  role: string;
  setting: string;
  coreIdentity: string;
  backstory: string;
  boosted: boolean;
  onFieldChange: (field: string, value: string) => void;
  onNext: () => void;
};

export function CharacterIdentityStep({
  name,
  era,
  role,
  setting,
  coreIdentity,
  backstory,
  boosted,
  onFieldChange,
  onNext,
}: Props) {
  const canContinue = name.trim().length > 0;

  return (
    <div className="space-y-4 px-4 py-6">
      <div className="flex items-center gap-2">
        <h2 className="text-lg font-semibold text-white">Identity</h2>
        {boosted && (
          <span className="rounded-full bg-indigo-500/20 px-2 py-0.5 text-[10px] font-medium text-indigo-300">
            AI Generated
          </span>
        )}
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">Name *</label>
        <input
          type="text"
          value={name}
          onChange={(e) => onFieldChange("name", e.target.value)}
          placeholder="Character name"
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
        />
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Era</label>
          <input
            type="text"
            value={era}
            onChange={(e) => onFieldChange("era", e.target.value)}
            placeholder="e.g. modern, Victorian"
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Role</label>
          <input
            type="text"
            value={role}
            onChange={(e) => onFieldChange("role", e.target.value)}
            placeholder="e.g. Detective, Scientist"
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">Setting</label>
        <textarea
          value={setting}
          onChange={(e) => onFieldChange("setting", e.target.value)}
          placeholder="Describe where the character lives (first person)..."
          rows={2}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">Core Identity</label>
        <textarea
          value={coreIdentity}
          onChange={(e) => onFieldChange("coreIdentity", e.target.value)}
          placeholder="Who is this character at their core? (first person, 3-5 sentences)"
          rows={3}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">Backstory</label>
        <textarea
          value={backstory}
          onChange={(e) => onFieldChange("backstory", e.target.value)}
          placeholder="Life story and key events (first person)..."
          rows={4}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>

      <button
        onClick={onNext}
        disabled={!canContinue}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50"
      >
        Continue
      </button>
    </div>
  );
}
