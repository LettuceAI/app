import { useState, useEffect, useCallback, useRef } from "react";
import {
  Network,
  Copy,
  RefreshCw,
  Shield,
  Server,
  Check,
  Info,
  Loader2,
  Radio,
  Eye,
  EyeOff,
  ChevronDown,
  Plus,
  X,
  Settings2,
} from "lucide-react";
import {
  readSettings,
  saveAdvancedSettings,
  getHostApiStatus,
  startHostApi,
  stopHostApi,
  type HostApiStatus,
} from "../../../core/storage/repo";
import type { HostApiSettings, Model } from "../../../core/storage/schemas";
import { cn, colors } from "../../design-tokens";
import { BottomMenu } from "../../components/BottomMenu";
import { getProviderIcon } from "../../../core/utils/providerIcons";

function createDefaultHostApiSettings(): HostApiSettings {
  return {
    enabled: false,
    bindAddress: "0.0.0.0",
    port: 3333,
    token: "",
    exposedModels: [],
  };
}

function sanitizeHostModelId(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized.length > 0 ? normalized : "model";
}

export function HostApiPage() {
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [hostApi, setHostApi] = useState<HostApiSettings>(createDefaultHostApiSettings());
  const [hostApiStatus, setHostApiStatus] = useState<HostApiStatus | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [showToken, setShowToken] = useState(false);
  const loadedRef = useRef(false);

  // Model selector state
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [modelSearchQuery, setModelSearchQuery] = useState("");
  // Model config editor state
  const [editingModelId, setEditingModelId] = useState<string | null>(null);

  useEffect(() => {
    if (loadedRef.current) return;
    loadedRef.current = true;

    (async () => {
      try {
        const [settings, status] = await Promise.all([
          readSettings(),
          getHostApiStatus(),
        ]);
        setModels(settings.models?.filter((m) => m.outputScopes.includes("text")) ?? []);
        setHostApi(settings.advancedSettings?.hostApi ?? createDefaultHostApiSettings());
        setHostApiStatus(status);
      } catch (err) {
        console.error("Failed to load settings:", err);
      } finally {
        setIsLoading(false);
      }
    })();
  }, []);

  const handleCopy = useCallback(async (value: string, label: string) => {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      setTimeout(() => setCopied(null), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }, []);

  const handleGenerateToken = () => {
    const bytes = new Uint8Array(24);
    globalThis.crypto.getRandomValues(bytes);
    const token = Array.from(bytes, (v) => v.toString(16).padStart(2, "0")).join("");
    setHostApi((c) => ({ ...c, token }));
  };

  const getExposedModel = (modelId: string) =>
    hostApi.exposedModels.find((e) => e.modelId === modelId);

  const addModel = (model: Model) => {
    const existing = getExposedModel(model.id);
    if (existing) {
      if (!existing.enabled) {
        setHostApi((c) => ({
          ...c,
          exposedModels: c.exposedModels.map((e) =>
            e.modelId === model.id ? { ...e, enabled: true } : e,
          ),
        }));
      }
      return;
    }
    setHostApi((c) => ({
      ...c,
      exposedModels: [
        ...c.exposedModels,
        {
          id: sanitizeHostModelId(model.displayName),
          modelId: model.id,
          enabled: true,
          label: model.displayName,
        },
      ],
    }));
  };

  const removeModel = (modelId: string) => {
    setHostApi((c) => ({
      ...c,
      exposedModels: c.exposedModels.filter((e) => e.modelId !== modelId),
    }));
    if (editingModelId === modelId) setEditingModelId(null);
  };

  const updateExposedField = (
    modelId: string,
    field: "id" | "label",
    value: string,
  ) => {
    setHostApi((c) => ({
      ...c,
      exposedModels: c.exposedModels.map((e) =>
        e.modelId === modelId
          ? { ...e, [field]: field === "id" ? sanitizeHostModelId(value) : value }
          : e,
      ),
    }));
  };

  const handleApply = async () => {
    setSaving(true);
    setError(null);
    try {
      const settings = await readSettings();
      const advanced = settings.advancedSettings ?? { creationHelperEnabled: false, helpMeReplyEnabled: true };
      advanced.hostApi = hostApi;
      await saveAdvancedSettings(advanced);
      const status = hostApi.enabled ? await startHostApi() : await stopHostApi();
      setHostApiStatus(status);
    } catch (err) {
      console.error("Failed to save host API settings:", err);
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleRefreshStatus = async () => {
    try {
      setHostApiStatus(await getHostApiStatus());
    } catch {
      // ignore
    }
  };

  const enabledModels = hostApi.exposedModels.filter((e) => e.enabled);
  const resolveModel = (modelId: string) => models.find((m) => m.id === modelId);
  const isRunning = hostApiStatus?.running ?? false;

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-fg/20 border-t-fg/60" />
      </div>
    );
  }

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-6">
          {/* Status Banner */}
          <div
            className={cn(
              "rounded-xl border p-3.5",
              isRunning ? "border-accent/25 bg-accent/8" : "border-fg/10 bg-fg/5",
            )}
          >
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <div
                  className={cn(
                    "relative flex h-10 w-10 items-center justify-center rounded-xl border",
                    isRunning ? "border-accent/40 bg-accent/15" : "border-fg/15 bg-fg/8",
                  )}
                >
                  <Radio
                    className={cn("h-5 w-5", isRunning ? "text-accent" : "text-fg/40")}
                  />
                  {isRunning && (
                    <span className="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-surface bg-accent" />
                  )}
                </div>
                <div>
                  <p className="text-sm font-medium text-fg">
                    {isRunning ? "Server Running" : "Server Stopped"}
                  </p>
                  {isRunning && hostApiStatus?.baseUrl ? (
                    <button
                      onClick={() => handleCopy(hostApiStatus.baseUrl ?? "", "url")}
                      className="flex items-center gap-1.5 text-[11px] text-fg/50 transition hover:text-fg/70"
                    >
                      <span className="font-mono">{hostApiStatus.baseUrl}</span>
                      {copied === "url" ? (
                        <Check className="h-3 w-3 text-accent" />
                      ) : (
                        <Copy className="h-3 w-3" />
                      )}
                    </button>
                  ) : !isRunning ? (
                    <p className="text-[11px] text-fg/40">
                      {hostApi.enabled
                        ? "Apply changes to start the server"
                        : "Enable and apply to start"}
                    </p>
                  ) : null}
                </div>
              </div>
              <button
                onClick={handleRefreshStatus}
                className={cn(
                  "rounded-lg border border-fg/10 bg-fg/5 p-2 text-fg/50 transition",
                  "hover:border-fg/20 hover:bg-fg/10 hover:text-fg/70",
                )}
                aria-label="Refresh status"
              >
                <RefreshCw className="h-4 w-4" />
              </button>
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left Column - Server Configuration */}
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                Server Configuration
              </h3>

              {/* Enable Toggle */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                      <Server className="h-4 w-4 text-info" />
                    </div>
                    <div>
                      <span className="text-sm font-medium text-fg">Enable Host API</span>
                      <p className="text-[11px] text-fg/45">
                        Start an OpenAI-compatible server on your LAN
                      </p>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={hostApi.enabled}
                      onChange={(e) => setHostApi((c) => ({ ...c, enabled: e.target.checked }))}
                      className="sr-only peer"
                    />
                    <div
                      className={cn(
                        "w-9 h-5 rounded-full transition-colors",
                        hostApi.enabled ? "bg-info" : "bg-fg/20",
                      )}
                    >
                      <div
                        className={cn(
                          "absolute top-0.5 left-0.5 w-4 h-4 bg-fg rounded-full transition-transform shadow-sm",
                          hostApi.enabled && "translate-x-4",
                        )}
                      />
                    </div>
                  </label>
                </div>
              </div>

              {/* Network Settings */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-4 space-y-4">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <Network className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Network</h3>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.16em] text-fg/35">
                      Bind Address
                    </label>
                    <input
                      value={hostApi.bindAddress}
                      onChange={(e) => setHostApi((c) => ({ ...c, bindAddress: e.target.value }))}
                      className={cn(
                        "w-full rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-2",
                        "font-mono text-sm text-fg focus:border-fg/30 focus:outline-none",
                      )}
                    />
                  </div>
                  <div>
                    <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-[0.16em] text-fg/35">
                      Port
                    </label>
                    <input
                      type="number"
                      min={1}
                      max={65535}
                      value={hostApi.port}
                      onChange={(e) =>
                        setHostApi((c) => ({
                          ...c,
                          port: Math.max(1, Math.min(65535, Number(e.target.value) || 3333)),
                        }))
                      }
                      className={cn(
                        "w-full rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-2",
                        "font-mono text-sm text-fg focus:border-fg/30 focus:outline-none",
                      )}
                    />
                  </div>
                </div>
              </div>

              {/* Authentication */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-4 space-y-3">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                      <Shield className="h-4 w-4 text-warning" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">Authentication</h3>
                  </div>
                  <button
                    onClick={handleGenerateToken}
                    className={cn(
                      "rounded-lg border border-info/20 bg-info/8 px-3 py-1.5",
                      "text-[11px] font-medium text-info/80 transition",
                      "hover:border-info/35 hover:bg-info/14",
                    )}
                  >
                    Generate Token
                  </button>
                </div>

                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <input
                      type={showToken ? "text" : "password"}
                      value={hostApi.token}
                      onChange={(e) => setHostApi((c) => ({ ...c, token: e.target.value }))}
                      className={cn(
                        "w-full rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-2 pr-9",
                        "font-mono text-sm text-fg focus:border-fg/30 focus:outline-none",
                      )}
                      placeholder="Bearer token for API access"
                    />
                    <button
                      onClick={() => setShowToken(!showToken)}
                      className="absolute right-2.5 top-1/2 -translate-y-1/2 text-fg/35 transition hover:text-fg/60"
                    >
                      {showToken ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                    </button>
                  </div>
                  <button
                    onClick={() => handleCopy(hostApi.token, "token")}
                    className={cn(
                      "rounded-lg border border-fg/10 bg-fg/5 px-3 text-fg/50 transition",
                      "hover:border-fg/20 hover:bg-fg/10 hover:text-fg/70",
                    )}
                    aria-label="Copy token"
                  >
                    {copied === "token" ? (
                      <Check className="h-4 w-4 text-accent" />
                    ) : (
                      <Copy className="h-4 w-4" />
                    )}
                  </button>
                </div>
                {!hostApi.token && (
                  <p className="text-[11px] text-warning/70">
                    A token is recommended to prevent unauthorized access to your models.
                  </p>
                )}
              </div>
            </div>

            {/* Right Column - Exposed Models */}
            <div className="space-y-4">
              <div className="flex items-center justify-between px-1">
                <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                  Exposed Models
                </h3>
                <span className="text-[11px] text-fg/40">
                  {enabledModels.length} model{enabledModels.length !== 1 ? "s" : ""} selected
                </span>
              </div>

              {/* Add Model Button */}
              <button
                type="button"
                onClick={() => setShowModelSelector(true)}
                className="flex w-full items-center justify-between rounded-xl border border-dashed border-fg/15 bg-surface-el/20 px-3.5 py-3 text-left transition hover:border-fg/25 hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
              >
                <div className="flex items-center gap-2.5">
                  <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-info/25 bg-info/10">
                    <Plus className="h-4 w-4 text-info" />
                  </div>
                  <span className="text-sm text-fg/50">Add models to expose...</span>
                </div>
                <ChevronDown className="h-4 w-4 text-fg/30" />
              </button>

              {/* Selected Models List */}
              {enabledModels.length === 0 ? (
                <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-6 text-center">
                  <Server className="mx-auto h-7 w-7 text-fg/15" />
                  <p className="mt-2 text-sm text-fg/35">No models exposed yet</p>
                  <p className="mt-1 text-[11px] text-fg/25">
                    Add models above to make them available via the API
                  </p>
                </div>
              ) : (
                <div className="space-y-2">
                  {enabledModels.map((exposed) => {
                    const model = resolveModel(exposed.modelId);
                    const isEditing = editingModelId === exposed.modelId;
                    return (
                      <div
                        key={exposed.modelId}
                        className={cn(
                          "rounded-xl border px-3.5 py-3 transition-all",
                          isEditing ? "border-info/25 bg-info/5" : "border-fg/10 bg-fg/5",
                        )}
                      >
                        <div className="flex items-center gap-3">
                          <div className="shrink-0">
                            {model ? getProviderIcon(model.providerId) : (
                              <Server className="h-5 w-5 text-fg/30" />
                            )}
                          </div>
                          <div className="min-w-0 flex-1">
                            <p className="truncate text-sm font-medium text-fg">
                              {model?.displayName ?? exposed.label ?? "Unknown Model"}
                            </p>
                            <p className="truncate text-[11px] text-fg/40">
                              {model?.providerLabel ?? "Provider unavailable"}
                              {exposed.id && (
                                <span className="text-fg/25"> &middot; <span className="font-mono">{exposed.id}</span></span>
                              )}
                            </p>
                          </div>
                          <div className="flex shrink-0 items-center gap-1">
                            <button
                              onClick={() => setEditingModelId(isEditing ? null : exposed.modelId)}
                              className={cn(
                                "rounded-lg p-1.5 transition",
                                isEditing
                                  ? "bg-info/15 text-info"
                                  : "text-fg/30 hover:bg-fg/10 hover:text-fg/60",
                              )}
                              aria-label="Configure model"
                            >
                              <Settings2 className="h-3.5 w-3.5" />
                            </button>
                            <button
                              onClick={() => removeModel(exposed.modelId)}
                              className="rounded-lg p-1.5 text-fg/30 transition hover:bg-danger/10 hover:text-danger/70"
                              aria-label="Remove model"
                            >
                              <X className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        </div>

                        {/* Expandable config */}
                        {isEditing && (
                          <div className="mt-3 space-y-2.5 border-t border-fg/8 pt-3">
                            <div>
                              <label className="mb-1 block text-[10px] font-medium uppercase tracking-[0.16em] text-fg/30">
                                API Model ID
                              </label>
                              <input
                                value={exposed.id}
                                onChange={(e) => updateExposedField(exposed.modelId, "id", e.target.value)}
                                className={cn(
                                  "w-full rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-1.5",
                                  "font-mono text-[12px] text-fg focus:border-fg/30 focus:outline-none",
                                )}
                                placeholder="api-model-id"
                              />
                              <p className="mt-1 text-[10px] text-fg/25">
                                The model name clients will use in API requests
                              </p>
                            </div>
                            <div>
                              <label className="mb-1 block text-[10px] font-medium uppercase tracking-[0.16em] text-fg/30">
                                Display Label
                              </label>
                              <input
                                value={exposed.label ?? ""}
                                onChange={(e) => updateExposedField(exposed.modelId, "label", e.target.value)}
                                className={cn(
                                  "w-full rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-1.5",
                                  "text-[12px] text-fg focus:border-fg/30 focus:outline-none",
                                )}
                                placeholder="Friendly name shown in /v1/models"
                              />
                            </div>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          {/* Error */}
          {error && (
            <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-[12px] text-danger/85">
              {error}
            </div>
          )}

          {/* Apply Button */}
          <button
            onClick={handleApply}
            disabled={saving}
            className={cn(
              "w-full rounded-xl border px-4 py-3 text-sm font-medium transition-all",
              saving
                ? "border-fg/10 bg-fg/5 text-fg/30 cursor-not-allowed"
                : "border-info/30 bg-info/12 text-info hover:border-info/45 hover:bg-info/18",
            )}
          >
            {saving ? (
              <span className="flex items-center justify-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                Applying...
              </span>
            ) : (
              "Apply & Restart Server"
            )}
          </button>

          {/* Info Card */}
          <div
            className={cn(
              "rounded-xl border px-4 py-3.5",
              colors.glass.subtle,
              "flex items-start gap-3",
            )}
          >
            <Info className="mt-0.5 h-4 w-4 shrink-0 text-fg/30" />
            <div className="text-[11px] leading-relaxed text-fg/45 space-y-1.5">
              <p>
                Exposes an <strong className="text-fg/60">OpenAI-compatible API</strong> on your
                local network so other devices and apps can use your configured models.
              </p>
              <p>
                Endpoints: <code className="text-fg/50">/health</code>,{" "}
                <code className="text-fg/50">/v1/models</code>,{" "}
                <code className="text-fg/50">/v1/chat/completions</code>
              </p>
            </div>
          </div>
        </div>
      </main>

      {/* Model Selection BottomMenu */}
      <BottomMenu
        isOpen={showModelSelector}
        onClose={() => {
          setShowModelSelector(false);
          setModelSearchQuery("");
        }}
        title="Select Models to Expose"
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={modelSearchQuery}
              onChange={(e) => setModelSearchQuery(e.target.value)}
              placeholder="Search models..."
              className="w-full rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none"
            />
            <svg
              className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>

          <div className="space-y-2 max-h-[50vh] overflow-y-auto">
            {models
              .filter((model) => {
                if (!modelSearchQuery) return true;
                const q = modelSearchQuery.toLowerCase();
                return (
                  model.displayName?.toLowerCase().includes(q) ||
                  model.name?.toLowerCase().includes(q) ||
                  model.providerLabel?.toLowerCase().includes(q)
                );
              })
              .map((model) => {
                const isSelected = getExposedModel(model.id)?.enabled ?? false;
                return (
                  <button
                    key={model.id}
                    onClick={() => {
                      if (isSelected) {
                        removeModel(model.id);
                      } else {
                        addModel(model);
                      }
                    }}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                      isSelected
                        ? "border-info/40 bg-info/10"
                        : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                    )}
                  >
                    {getProviderIcon(model.providerId)}
                    <div className="flex-1 min-w-0">
                      <span className="block truncate text-sm text-fg">
                        {model.displayName || model.name}
                      </span>
                      <span className="block truncate text-xs text-fg/40">
                        {model.providerLabel}
                      </span>
                    </div>
                    {isSelected && <Check className="h-4 w-4 shrink-0 text-info" />}
                  </button>
                );
              })}

            {models.length === 0 && (
              <div className="py-6 text-center">
                <p className="text-sm text-fg/40">No text-capable models configured</p>
                <p className="mt-1 text-[11px] text-fg/30">
                  Add models in Settings first
                </p>
              </div>
            )}
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
