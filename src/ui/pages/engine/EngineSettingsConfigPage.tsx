import { useParams, useNavigate } from "react-router-dom";
import { Check, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import { useEngineSettingsConfigController } from "./hooks/useEngineConfigController";
import { readSettings } from "../../../core/storage/repo";
import type { ProviderCredential } from "../../../core/storage/schemas";

export function EngineSettingsConfigPage() {
  const { credentialId } = useParams<{ credentialId: string }>();
  const navigate = useNavigate();
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
        <p className="text-sm text-white/60">Engine provider not found.</p>
        <button
          onClick={() => navigate(-1)}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/70 hover:bg-white/10"
        >
          Go Back
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
      <button
        onClick={() => onChange(!value)}
        className={`relative h-6 w-11 shrink-0 rounded-full transition ${
          value ? "bg-emerald-500/40" : "bg-white/10"
        }`}
      >
        <span
          className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
            value ? "translate-x-5" : "translate-x-0"
          }`}
        />
      </button>
    </div>
  );
}

// ── Main content ───────────────────────────────────────────────────────────

function SettingsInner({ credential }: { credential: ProviderCredential }) {
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
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">Engine</h3>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-white/70">Data Directory</label>
              <input
                type="text"
                value={v.dataDir}
                onChange={(e) => update({ dataDir: e.target.value })}
                className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-white/70">Log Level</label>
              <select
                value={v.logLevel}
                onChange={(e) => update({ logLevel: e.target.value })}
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
              value={v.maxHistory}
              onChange={(val) => update({ maxHistory: Math.max(1, Math.round(val)) })}
              min={1}
            />
          </section>

          {/* Background Loops */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">Background Loops</h3>
            <div className="grid grid-cols-2 gap-3">
              <NumberField
                label="Synthesis (min)"
                value={v.synthesisInterval}
                onChange={(val) => update({ synthesisInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
              <NumberField
                label="Consolidation (min)"
                value={v.consolidationInterval}
                onChange={(val) => update({ consolidationInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
              <NumberField
                label="BM25 Rebuild (min)"
                value={v.bm25RebuildInterval}
                onChange={(val) => update({ bm25RebuildInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
              <NumberField
                label="Drip Research (min)"
                value={v.dripResearchInterval}
                onChange={(val) => update({ dripResearchInterval: Math.max(1, Math.round(val)) })}
                min={1}
              />
            </div>
          </section>

          {/* Memory Config */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">Memory</h3>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-white/70">Embedding Model</label>
              <input
                type="text"
                value={v.embeddingModel}
                onChange={(e) => update({ embeddingModel: e.target.value })}
                className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
              />
            </div>
            <NumberField
              label="Max Retrieval Results"
              value={v.maxRetrievalResults}
              onChange={(val) => update({ maxRetrievalResults: Math.max(1, Math.round(val)) })}
              min={1}
            />
            <SliderField
              label="Dense Weight"
              value={v.denseWeight}
              onChange={(val) => update({ denseWeight: val })}
            />
            <SliderField
              label="BM25 Weight"
              value={v.bm25Weight}
              onChange={(val) => update({ bm25Weight: val })}
            />
            <SliderField
              label="Graph Weight"
              value={v.graphWeight}
              onChange={(val) => update({ graphWeight: val })}
            />
            <NumberField
              label="Recency Boost (hours)"
              value={v.recencyBoostHours}
              onChange={(val) => update({ recencyBoostHours: Math.max(0, val) })}
              min={0}
              step={0.5}
            />
            <SliderField
              label="Random Surface Probability"
              value={v.randomSurfaceProbability}
              onChange={(val) => update({ randomSurfaceProbability: val })}
            />
          </section>

          {/* Safety */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">Safety</h3>
            <ToggleField
              label="Honesty Section"
              description="Include honesty section in system prompt"
              value={v.honestySection}
              onChange={(val) => update({ honestySection: val })}
            />
            <ToggleField
              label="User Data Deletion"
              description="Allow users to request data deletion"
              value={v.userDataDeletion}
              onChange={(val) => update({ userDataDeletion: val })}
            />
          </section>

          {/* Research */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-white/40">Research</h3>
            <ToggleField
              label="Scrape on Boot"
              description="Run research scrape on engine startup"
              value={v.initialScrapeOnBoot}
              onChange={(val) => update({ initialScrapeOnBoot: val })}
            />
            <NumberField
              label="Periodic Interval (hours)"
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
                <Loader2 className="h-4 w-4 animate-spin" /> Saving...
              </span>
            ) : saved ? (
              <span className="flex items-center justify-center gap-2">
                <Check className="h-4 w-4" /> Saved
              </span>
            ) : (
              "Save Changes"
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
