import { useParams, useNavigate } from "react-router-dom";
import { ChevronDown, ChevronUp, Check, Import, Loader2, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { useEngineProvidersConfigController } from "./hooks/useEngineConfigController";
import { readSettings } from "../../../core/storage/repo";
import type { ProviderCredential } from "../../../core/storage/schemas";
import { ENGINE_LLM_PROVIDERS, type EngineLlmProviderId } from "../../../core/engine/types";
import { getProviderIcon } from "../../../core/utils/providerIcons";

export function EngineProvidersConfigPage() {
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

  return <ProvidersInner credential={credential} />;
}

function ProvidersInner({ credential }: { credential: ProviderCredential }) {
  const baseUrl = credential.baseUrl || "";
  const apiKey = credential.apiKey || "";
  const {
    state,
    appProviders,
    updateProvider,
    setDefaultBackend,
    importAppProvider,
    save,
  } = useEngineProvidersConfigController(baseUrl, apiKey);

  const [editing, setEditing] = useState<EngineLlmProviderId | null>(null);
  const [adding, setAdding] = useState<EngineLlmProviderId | null>(null);
  const [imported, setImported] = useState<Set<string>>(new Set());
  const [saved, setSaved] = useState(false);

  const handleImport = (provider: ProviderCredential) => {
    importAppProvider(provider);
    setImported((prev) => new Set(prev).add(provider.id));
  };

  const handleSave = async () => {
    const ok = await save();
    if (ok) {
      setSaved(true);
      setAdding(null);
      setTimeout(() => setSaved(false), 2000);
    }
  };

  const configured = useMemo(
    () => ENGINE_LLM_PROVIDERS.filter((p) => state.providers[p.id].enabled),
    [state.providers],
  );
  const unconfigured = useMemo(
    () => ENGINE_LLM_PROVIDERS.filter((p) => !state.providers[p.id].enabled),
    [state.providers],
  );

  const engineProviderIds: Set<string> = useMemo(
    () => new Set(ENGINE_LLM_PROVIDERS.map((p) => p.id)),
    [],
  );
  const importableProviders = useMemo(
    () => appProviders.filter(
      (p) => engineProviderIds.has(p.providerId) && !state.providers[p.providerId as EngineLlmProviderId]?.enabled,
    ),
    [appProviders, engineProviderIds, state.providers],
  );

  if (state.loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-y-auto">
        <div className="space-y-5 px-4 py-4">

          {/* ── Configured Providers ─────────────────────────────────── */}
          {configured.length > 0 && (
            <section>
              <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-white/40">
                Active Providers
              </h3>
              <div className="space-y-2">
                {configured.map((provider) => {
                  const config = state.providers[provider.id];
                  const isEditing = editing === provider.id;
                  const isDefault = state.defaultBackend === provider.id;

                  return (
                    <div
                      key={provider.id}
                      className="rounded-xl border border-white/10 bg-white/5 overflow-hidden"
                    >
                      {/* Summary row */}
                      <button
                        onClick={() => setEditing(isEditing ? null : provider.id)}
                        className="flex w-full items-center gap-3 px-4 py-3 text-left"
                      >
                        <div className="h-2.5 w-2.5 shrink-0 rounded-full bg-emerald-400" />
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-medium text-white">{provider.name}</p>
                          <p className="truncate text-[11px] text-white/40">
                            {config.model || "No model set"}
                          </p>
                        </div>
                        {isDefault && (
                          <span className="rounded-full bg-emerald-500/20 px-2 py-0.5 text-[10px] font-medium text-emerald-300">
                            Default
                          </span>
                        )}
                        {isEditing ? (
                          <ChevronUp className="h-4 w-4 text-white/40" />
                        ) : (
                          <ChevronDown className="h-4 w-4 text-white/40" />
                        )}
                      </button>

                      {/* Edit form */}
                      {isEditing && (
                        <ProviderForm
                          provider={provider}
                          config={config}
                          isDefault={isDefault}
                          onUpdate={(updates) => updateProvider(provider.id, updates)}
                          onSetDefault={() => setDefaultBackend(provider.id)}
                          onRemove={() => {
                            updateProvider(provider.id, { enabled: false });
                            setEditing(null);
                          }}
                        />
                      )}
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          {configured.length === 0 && (
            <div className="rounded-xl border border-amber-400/20 bg-amber-500/10 px-4 py-3">
              <p className="text-sm text-amber-200">No providers configured. Add at least one LLM backend below.</p>
            </div>
          )}

          {/* ── Add Provider ─────────────────────────────────────────── */}
          {unconfigured.length > 0 && (
            <section>
              <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-white/40">
                Add Provider
              </h3>

              {/* Import shortcut */}
              {importableProviders.length > 0 && (
                <div className="mb-3 rounded-xl border border-indigo-400/20 bg-indigo-500/10 p-3">
                  <div className="mb-2 flex items-center gap-2">
                    <Import className="h-3.5 w-3.5 text-indigo-300" />
                    <span className="text-xs font-medium text-indigo-200">Quick import from your app providers</span>
                  </div>
                  <div className="space-y-1.5">
                    {importableProviders.map((ap) => {
                      const isImported = imported.has(ap.id);
                      return (
                        <button
                          key={ap.id}
                          onClick={() => handleImport(ap)}
                          disabled={isImported}
                          className="flex w-full items-center gap-2.5 rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-left transition hover:border-white/20 hover:bg-black/30 disabled:opacity-60"
                        >
                          <div className="shrink-0 [&_svg]:h-4 [&_svg]:w-4">{getProviderIcon(ap.providerId)}</div>
                          <div className="flex-1 min-w-0">
                            <p className="truncate text-xs font-medium text-white">{ap.label}</p>
                            <p className="truncate text-[10px] text-white/40">{ap.providerId}</p>
                          </div>
                          {isImported ? (
                            <Check className="h-4 w-4 text-emerald-300" />
                          ) : (
                            <span className="rounded-full bg-indigo-500/20 px-2 py-0.5 text-[10px] font-medium text-indigo-300">
                              Import
                            </span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Available providers to add */}
              <div className="space-y-2">
                {unconfigured.map((provider) => {
                  const isAdding = adding === provider.id;
                  const config = state.providers[provider.id];

                  return (
                    <div
                      key={provider.id}
                      className="rounded-xl border border-white/10 bg-white/[0.02] overflow-hidden"
                    >
                      <button
                        onClick={() => setAdding(isAdding ? null : provider.id)}
                        className="flex w-full items-center gap-3 px-4 py-3 text-left"
                      >
                        <div className="h-2.5 w-2.5 shrink-0 rounded-full bg-white/20" />
                        <span className="flex-1 text-sm text-white/50">{provider.name}</span>
                        {isAdding ? (
                          <ChevronUp className="h-4 w-4 text-white/30" />
                        ) : (
                          <Plus className="h-4 w-4 text-white/30" />
                        )}
                      </button>

                      {isAdding && (
                        <ProviderForm
                          provider={provider}
                          config={config}
                          isDefault={false}
                          onUpdate={(updates) => updateProvider(provider.id, { ...updates, enabled: true })}
                          onSetDefault={() => {
                            updateProvider(provider.id, { enabled: true });
                            setDefaultBackend(provider.id);
                          }}
                          onEnable={() => {
                            updateProvider(provider.id, { enabled: true });
                            setAdding(null);
                          }}
                        />
                      )}
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          {state.error && <p className="text-xs font-medium text-rose-300">{state.error}</p>}

          <button
            onClick={() => void handleSave()}
            disabled={state.saving || configured.length === 0}
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

// ── Shared provider edit form ──────────────────────────────────────────────

function ProviderForm({
  provider,
  config,
  isDefault,
  onUpdate,
  onSetDefault,
  onRemove,
  onEnable,
}: {
  provider: (typeof ENGINE_LLM_PROVIDERS)[number];
  config: { model: string; apiKey: string; apiKeyRedacted: string; apiKeyChanged: boolean; baseUrl: string; maxTokens: number; temperature: number };
  isDefault: boolean;
  onUpdate: (updates: Record<string, unknown>) => void;
  onSetDefault: () => void;
  onRemove?: () => void;
  onEnable?: () => void;
}) {
  return (
    <div className="space-y-3 border-t border-white/10 px-4 py-3">
      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">Model</label>
        <input
          type="text"
          value={config.model}
          onChange={(e) => onUpdate({ model: e.target.value })}
          placeholder="e.g. claude-sonnet-4-5-20250929"
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
        />
      </div>

      {provider.requiresKey && (
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">API Key</label>
          <input
            type="password"
            value={config.apiKeyChanged ? config.apiKey : ""}
            onChange={(e) => onUpdate({ apiKey: e.target.value })}
            placeholder={config.apiKeyRedacted || "Enter your API key"}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
          {config.apiKeyRedacted && !config.apiKeyChanged && (
            <p className="mt-1 text-[10px] text-white/40">Current key: {config.apiKeyRedacted}</p>
          )}
        </div>
      )}

      {!provider.requiresKey && (
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Base URL</label>
          <input
            type="url"
            value={config.baseUrl}
            onChange={(e) => onUpdate({ baseUrl: e.target.value })}
            placeholder="http://localhost:11434"
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
      )}

      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Max Tokens</label>
          <input
            type="number"
            value={config.maxTokens}
            onChange={(e) => onUpdate({ maxTokens: Math.max(1, parseInt(e.target.value) || 1024) })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">Temperature</label>
          <input
            type="number"
            step="0.1"
            min="0"
            max="2"
            value={config.temperature}
            onChange={(e) => onUpdate({ temperature: Math.min(2, Math.max(0, parseFloat(e.target.value) || 0.9)) })}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
          />
        </div>
      </div>

      <div className="flex gap-2">
        {onEnable && (
          <button
            onClick={onEnable}
            className="flex-1 rounded-lg border border-emerald-400/30 bg-emerald-500/10 px-3 py-2 text-xs font-medium text-emerald-300 transition hover:bg-emerald-500/20"
          >
            Enable Provider
          </button>
        )}
        {!onEnable && !isDefault && (
          <button
            onClick={onSetDefault}
            className="flex-1 rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-xs font-medium text-white/60 transition hover:bg-white/10 hover:text-white"
          >
            Set as Default
          </button>
        )}
        {!onEnable && isDefault && (
          <div className="flex-1 rounded-lg border border-emerald-400/30 bg-emerald-500/10 px-3 py-2 text-center text-xs font-medium text-emerald-300">
            Default Backend
          </div>
        )}
        {onRemove && (
          <button
            onClick={onRemove}
            className="flex items-center gap-1.5 rounded-lg border border-rose-400/20 bg-rose-500/10 px-3 py-2 text-xs font-medium text-rose-300 transition hover:bg-rose-500/20"
          >
            <Trash2 className="h-3 w-3" /> Remove
          </button>
        )}
      </div>
    </div>
  );
}
