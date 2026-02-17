import { ChevronDown, ChevronUp, Check, Import } from "lucide-react";
import { useState } from "react";

import {
  ENGINE_LLM_PROVIDERS,
  type EngineLlmProviderId,
} from "../../../../core/engine/types";
import type { LlmProviderConfig } from "../hooks/engineSetupReducer";
import type { ProviderCredential } from "../../../../core/storage/schemas";
import { getProviderIcon } from "../../../../core/utils/providerIcons";

type Props = {
  llmProviders: Record<EngineLlmProviderId, LlmProviderConfig>;
  defaultBackend: EngineLlmProviderId;
  isSaving: boolean;
  error: string | null;
  appProviders: ProviderCredential[];
  onUpdate: (provider: EngineLlmProviderId, updates: Partial<LlmProviderConfig>) => void;
  onSetDefault: (provider: EngineLlmProviderId) => void;
  onImport: (provider: ProviderCredential) => void;
  onSave: () => Promise<boolean>;
  onNext: () => void;
};

export function ProvidersStep({
  llmProviders,
  defaultBackend,
  isSaving,
  error,
  appProviders,
  onUpdate,
  onSetDefault,
  onImport,
  onSave,
  onNext,
}: Props) {
  const [expanded, setExpanded] = useState<EngineLlmProviderId | null>(null);
  const [imported, setImported] = useState<Set<string>>(new Set());

  const handleImport = (provider: ProviderCredential) => {
    onImport(provider);
    setImported((prev) => new Set(prev).add(provider.id));
  };

  const handleSaveAndContinue = async () => {
    const ok = await onSave();
    if (ok) onNext();
  };

  const enabledCount = ENGINE_LLM_PROVIDERS.filter((p) => llmProviders[p.id].enabled).length;

  // Only show app providers that map to an engine LLM provider
  const engineProviderIds = new Set<string>(ENGINE_LLM_PROVIDERS.map((p) => p.id));
  const importableProviders = appProviders.filter((p) => engineProviderIds.has(p.providerId));

  return (
    <div className="space-y-4 px-4 py-6">
      <div>
        <h2 className="text-lg font-semibold text-white">LLM Provider</h2>
        <p className="mt-1 text-sm text-white/50">
          The Engine needs at least one LLM backend to function. Configure one or more providers
          below.
        </p>
      </div>

      {importableProviders.length > 0 && (
        <div className="rounded-xl border border-indigo-400/20 bg-indigo-500/10 p-3">
          <div className="mb-2 flex items-center gap-2">
            <Import className="h-3.5 w-3.5 text-indigo-300" />
            <span className="text-xs font-medium text-indigo-200">Import from your providers</span>
          </div>
          <div className="space-y-1.5">
            {importableProviders.map((provider) => {
              const isImported = imported.has(provider.id);
              return (
                <button
                  key={provider.id}
                  onClick={() => handleImport(provider)}
                  disabled={isImported}
                  className="flex w-full items-center gap-2.5 rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-left transition hover:border-white/20 hover:bg-black/30 disabled:opacity-60"
                >
                  <div className="shrink-0 [&_svg]:h-4 [&_svg]:w-4">{getProviderIcon(provider.providerId)}</div>
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-xs font-medium text-white">{provider.label}</p>
                    <p className="truncate text-[10px] text-white/40">{provider.providerId}</p>
                  </div>
                  {isImported ? (
                    <span className="rounded-full bg-emerald-500/20 px-2 py-0.5 text-[10px] font-medium text-emerald-300">
                      Imported
                    </span>
                  ) : (
                    <span className="rounded-full bg-indigo-500/20 px-2 py-0.5 text-[10px] font-medium text-indigo-300">
                      Use
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div className="space-y-2">
        {ENGINE_LLM_PROVIDERS.map((provider) => {
          const config = llmProviders[provider.id];
          const isExpanded = expanded === provider.id;
          const isDefault = defaultBackend === provider.id && config.enabled;

          return (
            <div
              key={provider.id}
              className="rounded-xl border border-white/10 bg-white/5 overflow-hidden"
            >
              {/* Header */}
              <button
                onClick={() => setExpanded(isExpanded ? null : provider.id)}
                className="flex w-full items-center gap-3 px-4 py-3 text-left"
              >
                <div
                  onClick={(e) => {
                    e.stopPropagation();
                    onUpdate(provider.id, { enabled: !config.enabled });
                  }}
                  className={`flex h-5 w-5 shrink-0 items-center justify-center rounded border transition ${
                    config.enabled
                      ? "border-emerald-400/60 bg-emerald-500/30"
                      : "border-white/20 bg-white/5"
                  }`}
                >
                  {config.enabled && <Check className="h-3 w-3 text-emerald-200" />}
                </div>
                <span className="flex-1 text-sm font-medium text-white">{provider.name}</span>
                {isDefault && (
                  <span className="rounded-full bg-emerald-500/20 px-2 py-0.5 text-[10px] font-medium text-emerald-300">
                    Default
                  </span>
                )}
                {isExpanded ? (
                  <ChevronUp className="h-4 w-4 text-white/40" />
                ) : (
                  <ChevronDown className="h-4 w-4 text-white/40" />
                )}
              </button>

              {/* Expanded Config */}
              {isExpanded && (
                <div className="space-y-3 border-t border-white/10 px-4 py-3">
                  <div>
                    <label className="mb-1 block text-[11px] font-medium text-white/70">
                      Model
                    </label>
                    <input
                      type="text"
                      value={config.model}
                      onChange={(e) => onUpdate(provider.id, { model: e.target.value })}
                      placeholder="e.g. claude-sonnet-4-5-20250929"
                      className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                    />
                  </div>

                  {provider.requiresKey && (
                    <div>
                      <label className="mb-1 block text-[11px] font-medium text-white/70">
                        API Key
                      </label>
                      <input
                        type="password"
                        value={config.apiKey}
                        onChange={(e) => onUpdate(provider.id, { apiKey: e.target.value })}
                        placeholder="Enter your API key"
                        className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                      />
                    </div>
                  )}

                  {!provider.requiresKey && (
                    <div>
                      <label className="mb-1 block text-[11px] font-medium text-white/70">
                        Base URL
                      </label>
                      <input
                        type="url"
                        value={config.baseUrl}
                        onChange={(e) => onUpdate(provider.id, { baseUrl: e.target.value })}
                        placeholder="http://localhost:11434"
                        className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                      />
                    </div>
                  )}

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="mb-1 block text-[11px] font-medium text-white/70">
                        Max Tokens
                      </label>
                      <input
                        type="number"
                        value={config.maxTokens}
                        onChange={(e) =>
                          onUpdate(provider.id, {
                            maxTokens: Math.max(1, parseInt(e.target.value) || 1024),
                          })
                        }
                        className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block text-[11px] font-medium text-white/70">
                        Temperature
                      </label>
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="2"
                        value={config.temperature}
                        onChange={(e) =>
                          onUpdate(provider.id, {
                            temperature: Math.min(2, Math.max(0, parseFloat(e.target.value) || 0.9)),
                          })
                        }
                        className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
                      />
                    </div>
                  </div>

                  {config.enabled && (
                    <button
                      onClick={() => onSetDefault(provider.id)}
                      disabled={isDefault}
                      className={`w-full rounded-lg px-3 py-2 text-xs font-medium transition ${
                        isDefault
                          ? "border border-emerald-400/30 bg-emerald-500/10 text-emerald-300"
                          : "border border-white/10 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white"
                      }`}
                    >
                      {isDefault ? "Default Backend" : "Set as Default"}
                    </button>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {error && <p className="text-xs font-medium text-rose-300">{error}</p>}

      <button
        onClick={() => void handleSaveAndContinue()}
        disabled={isSaving || enabledCount === 0}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {isSaving ? "Saving..." : "Save & Continue"}
      </button>
    </div>
  );
}
