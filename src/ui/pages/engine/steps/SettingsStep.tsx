import type { EngineSettings } from "../hooks/engineSetupReducer";

type Props = {
  settings: EngineSettings;
  isSaving: boolean;
  error: string | null;
  onUpdate: (updates: Partial<EngineSettings>) => void;
  onSave: () => Promise<boolean>;
};

function NumberField({
  label,
  value,
  onChange,
  min,
  max,
  step,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  return (
    <div>
      <label className="mb-1 block text-[11px] font-medium text-white/70">{label}</label>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
        className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
      />
    </div>
  );
}

function SliderField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <label className="text-[11px] font-medium text-white/70">{label}</label>
        <span className="text-[11px] text-white/50">{value.toFixed(2)}</span>
      </div>
      <input
        type="range"
        min="0"
        max="1"
        step="0.05"
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full accent-emerald-500"
      />
    </div>
  );
}

export function SettingsStep({ settings, isSaving, error, onUpdate, onSave }: Props) {
  return (
    <div className="space-y-5 px-4 py-6">
      <div>
        <h2 className="text-lg font-semibold text-white">Engine Settings</h2>
        <p className="mt-1 text-sm text-white/50">
          Configure engine-wide settings. These all have sensible defaults — feel free to skip.
        </p>
      </div>

      {/* Engine Config */}
      <div className="space-y-3">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">
          Engine
        </h3>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Data Directory</label>
          <input
            type="text"
            value={settings.dataDir}
            onChange={(e) => onUpdate({ dataDir: e.target.value })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Log Level</label>
          <select
            value={settings.logLevel}
            onChange={(e) => onUpdate({ logLevel: e.target.value })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          >
            <option value="DEBUG" className="bg-black">DEBUG</option>
            <option value="INFO" className="bg-black">INFO</option>
            <option value="WARNING" className="bg-black">WARNING</option>
            <option value="ERROR" className="bg-black">ERROR</option>
          </select>
        </div>
        <NumberField
          label="Max History (conversation turns)"
          value={settings.maxHistory}
          onChange={(v) => onUpdate({ maxHistory: Math.max(1, Math.round(v)) })}
          min={1}
        />
      </div>

      {/* Background Loops */}
      <div className="space-y-3">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">
          Background Loops
        </h3>
        <div className="grid grid-cols-2 gap-3">
          <NumberField
            label="Synthesis (min)"
            value={settings.synthesisInterval}
            onChange={(v) => onUpdate({ synthesisInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
          <NumberField
            label="Consolidation (min)"
            value={settings.consolidationInterval}
            onChange={(v) => onUpdate({ consolidationInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
          <NumberField
            label="BM25 Rebuild (min)"
            value={settings.bm25RebuildInterval}
            onChange={(v) => onUpdate({ bm25RebuildInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
          <NumberField
            label="Drip Research (min)"
            value={settings.dripResearchInterval}
            onChange={(v) => onUpdate({ dripResearchInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
        </div>
      </div>

      {/* Memory Config */}
      <div className="space-y-3">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">
          Memory
        </h3>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">
            Embedding Model
          </label>
          <input
            type="text"
            value={settings.embeddingModel}
            onChange={(e) => onUpdate({ embeddingModel: e.target.value })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
        <NumberField
          label="Max Retrieval Results"
          value={settings.maxRetrievalResults}
          onChange={(v) => onUpdate({ maxRetrievalResults: Math.max(1, Math.round(v)) })}
          min={1}
        />
        <SliderField
          label="Dense Weight"
          value={settings.denseWeight}
          onChange={(v) => onUpdate({ denseWeight: v })}
        />
        <SliderField
          label="BM25 Weight"
          value={settings.bm25Weight}
          onChange={(v) => onUpdate({ bm25Weight: v })}
        />
        <SliderField
          label="Graph Weight"
          value={settings.graphWeight}
          onChange={(v) => onUpdate({ graphWeight: v })}
        />
      </div>

      {error && <p className="text-xs font-medium text-rose-300">{error}</p>}

      <button
        onClick={() => void onSave()}
        disabled={isSaving}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {isSaving ? "Completing Setup..." : "Complete Setup"}
      </button>
    </div>
  );
}
