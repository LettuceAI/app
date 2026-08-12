import type { EngineSettings } from "../hooks/engineSetupReducer";
import { useI18n } from "../../../../core/i18n/context";
import { NumberInput } from "../../../components/NumberInput";

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
      <NumberInput
        value={value}
        min={min}
        max={max}
        step={step}
        decimals={step !== undefined && step < 1 ? 2 : 0}
        onChange={(next) => onChange(next ?? 0)}
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
  const { t } = useI18n();
  return (
    <div className="space-y-5 px-4 py-6">
      <div>
        <h2 className="text-lg font-semibold text-white">{t("engine.settingsStep.title")}</h2>
        <p className="mt-1 text-sm text-white/50">
          {t("engine.settingsStep.subtitle")}
        </p>
      </div>

      {/* Engine Config */}
      <div className="space-y-3">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">
          {t("engine.settings.sections.engine")}
        </h3>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.settings.fields.dataDirectory")}</label>
          <input
            type="text"
            value={settings.dataDir}
            onChange={(e) => onUpdate({ dataDir: e.target.value })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.settings.fields.logLevel")}</label>
          <select
            value={settings.logLevel}
            onChange={(e) => onUpdate({ logLevel: e.target.value })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          >
            <option value="DEBUG" className="bg-black">{t("engine.settings.logLevels.debug")}</option>
            <option value="INFO" className="bg-black">{t("engine.settings.logLevels.info")}</option>
            <option value="WARNING" className="bg-black">{t("engine.settings.logLevels.warning")}</option>
            <option value="ERROR" className="bg-black">{t("engine.settings.logLevels.error")}</option>
          </select>
        </div>
        <NumberField
          label={t("engine.settings.fields.maxHistory")}
          value={settings.maxHistory}
          onChange={(v) => onUpdate({ maxHistory: Math.max(1, Math.round(v)) })}
          min={1}
        />
      </div>

      {/* Background Loops */}
      <div className="space-y-3">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">
          {t("engine.settings.sections.backgroundLoops")}
        </h3>
        <div className="grid grid-cols-2 gap-3">
          <NumberField
            label={t("engine.settings.backgroundLoops.synthesis")}
            value={settings.synthesisInterval}
            onChange={(v) => onUpdate({ synthesisInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
          <NumberField
            label={t("engine.settings.backgroundLoops.consolidation")}
            value={settings.consolidationInterval}
            onChange={(v) => onUpdate({ consolidationInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
          <NumberField
            label={t("engine.settings.backgroundLoops.bm25Rebuild")}
            value={settings.bm25RebuildInterval}
            onChange={(v) => onUpdate({ bm25RebuildInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
          <NumberField
            label={t("engine.settings.backgroundLoops.dripResearch")}
            value={settings.dripResearchInterval}
            onChange={(v) => onUpdate({ dripResearchInterval: Math.max(1, Math.round(v)) })}
            min={1}
          />
        </div>
      </div>

      {/* Memory Config */}
      <div className="space-y-3">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">
          {t("engine.settings.sections.memory")}
        </h3>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">
            {t("engine.settings.memory.embeddingModel")}
          </label>
          <input
            type="text"
            value={settings.embeddingModel}
            onChange={(e) => onUpdate({ embeddingModel: e.target.value })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
        <NumberField
          label={t("engine.settings.memory.maxRetrieval")}
          value={settings.maxRetrievalResults}
          onChange={(v) => onUpdate({ maxRetrievalResults: Math.max(1, Math.round(v)) })}
          min={1}
        />
        <SliderField
          label={t("engine.settings.memory.denseWeight")}
          value={settings.denseWeight}
          onChange={(v) => onUpdate({ denseWeight: v })}
        />
        <SliderField
          label={t("engine.settings.memory.bm25Weight")}
          value={settings.bm25Weight}
          onChange={(v) => onUpdate({ bm25Weight: v })}
        />
        <SliderField
          label={t("engine.settings.memory.graphWeight")}
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
        {isSaving ? t("engine.settingsStep.completingSetup") : t("engine.settingsStep.completeSetup")}
      </button>
    </div>
  );
}
