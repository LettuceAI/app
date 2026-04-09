import { useParams, useNavigate } from "react-router-dom";
import { Check, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import { useEngineSettingsConfigController } from "./hooks/useEngineConfigController";
import { readSettings } from "../../../core/storage/repo";
import type { ProviderCredential } from "../../../core/storage/schemas";
import { useI18n } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";

export function EngineSettingsConfigPage() {
  const { credentialId } = useParams<{ credentialId: string }>();
  const navigate = useNavigate();
  const { t } = useI18n();
  const [credential, setCredential] = useState<ProviderCredential | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const settings = await readSettings();
        const cred = settings.providerCredentials.find((p) => p.id === credentialId);
        if (!cancelled && cred) setCredential(cred);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [credentialId]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (!credential) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4">
        <p className="text-sm text-white/60">{t("engine.errors.providerNotFound")}</p>
        <button
          onClick={() => navigate(-1)}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/70 hover:bg-white/10"
        >
          {t("common.buttons.goBack")}
        </button>
      </div>
    );
  }

  return <SettingsInner credential={credential} />;
}

// ── Shared field components ────────────────────────────────────────────────

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
  min = 0,
  max = 1,
  step = 0.05,
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
      <div className="mb-1 flex items-center justify-between">
        <label className="text-[11px] font-medium text-white/70">{label}</label>
        <span className="text-[11px] text-white/50">{value.toFixed(2)}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full accent-emerald-500"
      />
    </div>
  );
}

function ToggleField({
  label,
  description,
  value,
  onChange,
}: {
  label: string;
  description?: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="min-w-0">
        <p className="text-sm text-white/80">{label}</p>
        {description && <p className="text-[11px] text-white/40">{description}</p>}
      </div>
      <Switch checked={value} onChange={onChange} />
    </div>
  );
}

// ── Main content ───────────────────────────────────────────────────────────

function SettingsInner({ credential }: { credential: ProviderCredential }) {
  const { t } = useI18n();
  const baseUrl = credential.baseUrl || "";
  const apiKey = credential.apiKey || "";
  const { state, update, save } = useEngineSettingsConfigController(baseUrl, apiKey);
  const [saved, setSaved] = useState(false);

  const handleSave = async () => {
    const ok = await save();
    if (ok) {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    }
  };

  if (state.loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  const v = state.values;

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-y-auto">
        <div className="space-y-5 px-4 py-4">
          {/* Engine Config */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">{t("engine.settings.sections.engine")}</h3>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.settings.fields.dataDirectory")}</label>
              <input
                type="text"
                value={v.dataDir}
                onChange={(e) => update({ dataDir: e.target.value })}
                className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.settings.fields.logLevel")}</label>
              <select
                value={v.logLevel}
                onChange={(e) => update({ logLevel: e.target.value })}
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
              value={v.maxHistory}
              onChange={(val) => update({ maxHistory: Math.max(1, Math.round(val)) })}
              min={1}
            />
          </section>

          {/* Background Loops */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">{t("engine.settings.sections.backgroundLoops")}</h3>
            <div className="grid grid-cols-2 gap-3">
              <NumberField
                label={t("engine.settings.backgroundLoops.synthesis")}
                value={v.synthesisInterval}
                onChange={(val) => update({ synthesisInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
              <NumberField
                label={t("engine.settings.backgroundLoops.consolidation")}
                value={v.consolidationInterval}
                onChange={(val) => update({ consolidationInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
              <NumberField
                label={t("engine.settings.backgroundLoops.bm25Rebuild")}
                value={v.bm25RebuildInterval}
                onChange={(val) => update({ bm25RebuildInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
              <NumberField
                label={t("engine.settings.backgroundLoops.dripResearch")}
                value={v.dripResearchInterval}
                onChange={(val) => update({ dripResearchInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
            </div>
          </section>

          {/* Memory Config */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">{t("engine.settings.sections.memory")}</h3>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.settings.memory.embeddingModel")}</label>
              <input
                type="text"
                value={v.embeddingModel}
                onChange={(e) => update({ embeddingModel: e.target.value })}
                className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
              />
            </div>
            <NumberField
              label={t("engine.settings.memory.maxRetrieval")}
              value={v.maxRetrievalResults}
              onChange={(val) => update({ maxRetrievalResults: Math.max(1, Math.round(val)) })}
              min={1}
            />
            <SliderField
              label={t("engine.settings.memory.denseWeight")}
              value={v.denseWeight}
              onChange={(val) => update({ denseWeight: val })}
            />
            <SliderField
              label={t("engine.settings.memory.bm25Weight")}
              value={v.bm25Weight}
              onChange={(val) => update({ bm25Weight: val })}
            />
            <SliderField
              label={t("engine.settings.memory.graphWeight")}
              value={v.graphWeight}
              onChange={(val) => update({ graphWeight: val })}
            />
            <NumberField
              label={t("engine.settings.memory.recencyBoost")}
              value={v.recencyBoostHours}
              onChange={(val) => update({ recencyBoostHours: Math.max(0, val) })}
              min={0}
              step={0.5}
            />
            <SliderField
              label={t("engine.settings.memory.randomSurface")}
              value={v.randomSurfaceProbability}
              onChange={(val) => update({ randomSurfaceProbability: val })}
            />
          </section>

          {/* Safety */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">{t("engine.settings.sections.safety")}</h3>
            <ToggleField
              label={t("engine.settings.safety.honestySection")}
              description={t("engine.settings.safety.honestyDesc")}
              value={v.honestySection}
              onChange={(val) => update({ honestySection: val })}
            />
            <ToggleField
              label={t("engine.settings.safety.userDataDeletion")}
              description={t("engine.settings.safety.userDataDesc")}
              value={v.userDataDeletion}
              onChange={(val) => update({ userDataDeletion: val })}
            />
          </section>

          {/* Research */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">{t("engine.settings.sections.research")}</h3>
            <ToggleField
              label={t("engine.settings.research.scrapeOnBoot")}
              description={t("engine.settings.research.scrapeDesc")}
              value={v.initialScrapeOnBoot}
              onChange={(val) => update({ initialScrapeOnBoot: val })}
            />
            <NumberField
              label={t("engine.settings.research.periodicInterval")}
              value={v.periodicIntervalHours}
              onChange={(val) => update({ periodicIntervalHours: Math.max(1, Math.round(val)) })}
              min={1}
            />
          </section>

          {state.error && <p className="text-xs font-medium text-rose-300">{state.error}</p>}

          <button
            onClick={() => void handleSave()}
            disabled={state.saving}
            className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {state.saving ? (
              <span className="flex items-center justify-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" /> {t("engine.settings.saving")}
              </span>
            ) : saved ? (
              <span className="flex items-center justify-center gap-2">
                <Check className="h-4 w-4" /> {t("engine.settings.saved")}
              </span>
            ) : (
              t("engine.settings.saveChanges")
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
