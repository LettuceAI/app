import { AlertCircle, Loader, RefreshCw, ChevronDown } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TestResult } from "../hooks/onboardingReducer";
import type { ProviderCredential } from "../../../../core/storage/schemas";
import { getProviderIcon } from "../../../../core/utils/providerIcons";
import { ModelSelectionBottomMenu } from "../../../components/ModelSelectionBottomMenu";
import { Switch } from "../../../components/Switch";
import { useI18n } from "../../../../core/i18n/context";

interface ProviderConfigFormProps {
  selectedProviderId: string;
  selectedProviderName?: string;
  label: string;
  apiKey: string;
  baseUrl: string;
  config?: Record<string, any>;
  testResult: TestResult;
  isTesting: boolean;
  isSubmitting: boolean;
  canTest: boolean;
  canSave: boolean;
  onLabelChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  onBaseUrlChange: (value: string) => void;
  onConfigChange?: (config: Record<string, any>) => void;
  onTestConnection: () => void;
  onSave: () => void;
}

export function ProviderConfigForm({
  selectedProviderId,
  selectedProviderName,
  label,
  apiKey,
  baseUrl,
  config,
  testResult,
  isTesting,
  isSubmitting,
  canTest,
  canSave,
  onLabelChange,
  onApiKeyChange,
  onBaseUrlChange,
  onConfigChange,
  onTestConnection,
  onSave,
}: ProviderConfigFormProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const isCustomProvider = ["custom", "custom-anthropic"].includes(selectedProviderId);
  const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(selectedProviderId);
  const isHostProvider = selectedProviderId === "lettuce-host";
  const showBaseUrl = isCustomProvider || isLocalProvider || isHostProvider;

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <label className="text-xs font-medium text-white/70">{t("common.labels.name")}</label>
        <input
          type="text"
          value={label}
          onChange={(e) => onLabelChange(e.target.value)}
          placeholder={`My ${selectedProviderName || "Provider"}`}
          className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
        />
        <p className="text-[11px] text-gray-500">How this provider will appear in your menus</p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="text-xs font-medium text-white/70">
            API Key{isLocalProvider ? " (Optional)" : ""}
          </label>
          {!isLocalProvider && !isCustomProvider && (
            <button
              onClick={() =>
                navigate(
                  `/wheretofind${selectedProviderId ? `?provider=${selectedProviderId}` : ""}`,
                )
              }
              className="text-[11px] text-gray-400 hover:text-white transition-colors"
            >
              Where to find it
            </button>
          )}
        </div>
        <input
          type="text"
          value={apiKey}
          onChange={(e) => onApiKeyChange(e.target.value)}
          placeholder={isLocalProvider ? "Usually not required" : "sk-..."}
          className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
        />
        <p className="text-[11px] text-gray-500">Keys are encrypted locally</p>
      </div>

      {showBaseUrl && (
        <div className="space-y-2">
          <label className="text-xs font-medium text-white/70">Base URL</label>
          <input
            type="text"
            value={baseUrl}
            onChange={(e) => onBaseUrlChange(e.target.value)}
            placeholder={
              selectedProviderId === "intenserp"
                ? "http://127.0.0.1:7777/v1"
                : isHostProvider
                  ? "http://192.168.1.10:3333"
                  : isLocalProvider
                    ? "http://localhost:11434"
                    : "https://api.provider.com"
            }
            className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
          />
          <p className="text-[11px] text-gray-500">
            {isLocalProvider
              ? "Your local server address with port"
              : isHostProvider
                ? "Enter the desktop host URL shown by your host device"
                : "Override the default endpoint if needed"}
          </p>
        </div>
      )}

      {isCustomProvider && onConfigChange && (
        <>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <label className="text-xs font-medium text-white/70">Chat Endpoint</label>
              <input
                type="text"
                value={config?.chatEndpoint ?? "/v1/chat/completions"}
                onChange={(e) => onConfigChange({ ...config, chatEndpoint: e.target.value })}
                className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-medium text-white/70">System Role</label>
              <input
                type="text"
                value={config?.systemRole ?? "system"}
                onChange={(e) => onConfigChange({ ...config, systemRole: e.target.value })}
                className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <label className="text-xs font-medium text-white/70">User Role</label>
              <input
                type="text"
                value={config?.userRole ?? "user"}
                onChange={(e) => onConfigChange({ ...config, userRole: e.target.value })}
                className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-medium text-white/70">Assistant Role</label>
              <input
                type="text"
                value={config?.assistantRole ?? "assistant"}
                onChange={(e) => onConfigChange({ ...config, assistantRole: e.target.value })}
                className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
              />
            </div>
          </div>
          <div className="flex items-center justify-between pt-1">
            <span className="text-xs font-medium text-white/70">Supports Streaming</span>
            <Switch
              id="supportsStream-onboarding"
              checked={config?.supportsStream ?? true}
              onChange={(next) => onConfigChange({ ...config, supportsStream: next })}
            />
          </div>
          {selectedProviderId === "custom" && (
            <div className="space-y-2">
              <label className="text-xs font-medium text-white/70">Tool Choice Mode</label>
              <select
                value={config?.toolChoiceMode ?? "auto"}
                onChange={(e) => onConfigChange({ ...config, toolChoiceMode: e.target.value })}
                className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-white/30 focus:outline-none"
              >
                <option value="auto" className="bg-black">
                  Auto
                </option>
                <option value="required" className="bg-black">
                  Required
                </option>
                <option value="none" className="bg-black">
                  None
                </option>
                <option value="omit" className="bg-black">
                  Omit Field
                </option>
                <option value="passthrough" className="bg-black">
                  Passthrough (Tool Config)
                </option>
              </select>
              <p className="text-[11px] text-gray-500">
                Controls how `tool_choice` is sent to the custom endpoint.
              </p>
            </div>
          )}
          <div className="flex items-center justify-between pt-1">
            <span className="text-xs font-medium text-white/70">Merge Same-role Messages</span>
            <Switch
              id="mergeSameRoleMessages-onboarding"
              checked={config?.mergeSameRoleMessages ?? true}
              onChange={(next) => onConfigChange({ ...config, mergeSameRoleMessages: next })}
            />
          </div>
        </>
      )}

      {testResult && (
        <div
          className={`rounded-xl border px-4 py-3 text-sm ${
            testResult.success
              ? "border-emerald-400/40 bg-emerald-400/10 text-emerald-200"
              : "border-amber-400/40 bg-amber-400/10 text-amber-200"
          }`}
        >
          {testResult.message}
        </div>
      )}

      <div className="space-y-2 pt-2">
        <button
          onClick={onTestConnection}
          disabled={!canTest || isTesting}
          className="w-full min-h-11 rounded-xl border border-white/20 bg-white/10 px-4 py-3 font-medium text-white transition-all duration-200 hover:border-white/30 hover:bg-white/15 active:scale-[0.98] disabled:cursor-not-allowed disabled:border-white/5 disabled:bg-white/5 disabled:text-gray-500"
        >
          {isTesting ? (
            <div className="flex items-center justify-center gap-2">
              <Loader size={14} className="animate-spin" />
              Testing...
            </div>
          ) : (
            <div className="flex items-center justify-center gap-2">
              <AlertCircle size={14} />
              Test Connection
            </div>
          )}
        </button>

        <button
          onClick={onSave}
          disabled={!canSave || isSubmitting}
          className="w-full min-h-12 rounded-xl border border-emerald-400/40 bg-emerald-400/20 px-4 py-3 font-semibold text-emerald-100 transition-all duration-200 hover:border-emerald-300/80 hover:bg-emerald-400/30 active:scale-[0.98] disabled:cursor-not-allowed disabled:border-emerald-400/10 disabled:bg-emerald-400/5 disabled:text-emerald-400"
        >
          {isSubmitting ? (
            <div className="flex items-center justify-center gap-2">
              <Loader size={14} className="animate-spin" />
              Verifying...
            </div>
          ) : (
            "Continue"
          )}
        </button>
      </div>
    </div>
  );
}

interface ModelConfigFormProps {
  selectedCredential: ProviderCredential;
  displayName: string;
  modelName: string;
  error: string | null;
  isSaving: boolean;
  canSave: boolean;
  onDisplayNameChange: (value: string) => void;
  onModelNameChange: (value: string) => void;
  onSave: () => void;
  onSkip: () => void;
}

export function ModelConfigForm({
  selectedCredential,
  displayName,
  modelName,
  error,
  isSaving,
  canSave,
  onDisplayNameChange,
  onModelNameChange,
  onSave,
  onSkip,
}: ModelConfigFormProps) {
  const [fetchedModels, setFetchedModels] = useState<
    Array<{ id: string; displayName?: string; description?: string }>
  >([]);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [isManualInput, setIsManualInput] = useState(false);
  const [showModelSelector, setShowModelSelector] = useState(false);

  const isLocalModel = selectedCredential.providerId === "llamacpp";
  const modelFetchEnabled = !["llamacpp", "intenserp"].includes(selectedCredential.providerId);
  const modelIdLabel = isLocalModel ? "Model Path (GGUF)" : "Model ID";
  const modelIdPlaceholder = isLocalModel ? "/path/to/model.gguf" : "e.g. gpt-4o";

  const fetchModels = async () => {
    if (!modelFetchEnabled) {
      setFetchedModels([]);
      setIsManualInput(true);
      return;
    }
    setFetchingModels(true);
    try {
      const models = await invoke<any[]>("get_remote_models", {
        credentialId: selectedCredential.id,
      });
      const next = models ?? [];
      setFetchedModels(next);
      if (next.length > 0) {
        setIsManualInput(false);
      } else {
        setIsManualInput(true);
      }
    } catch (error) {
      console.error("Failed to fetch models:", error);
      setFetchedModels([]);
      setIsManualInput(true);
    } finally {
      setFetchingModels(false);
    }
  };

  const handleSelectModel = (modelId: string, nextDisplayName?: string) => {
    onModelNameChange(modelId);
    onDisplayNameChange(nextDisplayName || modelId);
    setShowModelSelector(false);
  };

  useEffect(() => {
    setFetchedModels([]);
    setIsManualInput(!modelFetchEnabled);
    if (modelFetchEnabled) {
      void fetchModels();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedCredential.id, selectedCredential.providerId, modelFetchEnabled]);

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <label className="text-xs font-medium text-white/70">Display Name</label>
        <input
          type="text"
          value={displayName}
          onChange={(e) => onDisplayNameChange(e.target.value)}
          placeholder="Creative mentor"
          className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
        />
        <p className="text-[11px] text-gray-500">How this model appears in menus</p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="text-[11px] font-bold tracking-wider text-white/50 uppercase">
            {modelIdLabel}
          </label>
          <div className="flex items-center gap-3">
            {!isLocalModel && modelFetchEnabled && fetchedModels.length > 0 && (
              <button
                type="button"
                onClick={() => setIsManualInput(!isManualInput)}
                className="text-[10px] uppercase font-bold tracking-wider text-white/40 hover:text-white/80 transition"
              >
                {isManualInput ? "Show List" : "Manual Input"}
              </button>
            )}
            {!isLocalModel && modelFetchEnabled && (
              <button
                type="button"
                onClick={fetchModels}
                disabled={fetchingModels}
                className="text-white/40 hover:text-white/80 transition disabled:opacity-30"
                title="Refresh model list"
              >
                <RefreshCw className={fetchingModels ? "animate-spin" : ""} size={14} />
              </button>
            )}
          </div>
        </div>

        {!isLocalModel && modelFetchEnabled && !isManualInput ? (
          <>
            <button
              type="button"
              onClick={() => setShowModelSelector(true)}
              className="w-full flex items-center justify-between rounded-xl border border-white/10 bg-black/20 px-4 py-3 text-white transition hover:bg-black/30 active:scale-[0.99]"
            >
              <span className={`block truncate ${!modelName ? "text-white/40" : ""}`}>
                {fetchedModels.find((m) => m.id === modelName)?.displayName ||
                  modelName ||
                  "Select a model..."}
              </span>
              <ChevronDown className="h-4 w-4 text-white/40" />
            </button>

            <ModelSelectionBottomMenu
              isOpen={showModelSelector}
              onClose={() => setShowModelSelector(false)}
              title="Select Model"
              models={fetchedModels as any}
              selectedModelIds={modelName ? [modelName] : []}
              searchPlaceholder="Search models..."
              theme="dark"
              tone="emerald"
              renderModelIcon={() => getProviderIcon(selectedCredential.providerId)}
              renderModelTitle={(model: any) => model.displayName || model.id}
              renderModelDescription={(model: any) => model.description || model.id}
              renderEmptyState={(query) => (
                <div className="py-12 text-center text-sm text-white/40">
                  No models found matching "{query}"
                </div>
              )}
              onSelectModel={(modelId) => {
                const model = fetchedModels.find((item) => item.id === modelId);
                handleSelectModel(modelId, model?.displayName);
              }}
            />
          </>
        ) : (
          <>
            <input
              type="text"
              value={modelName}
              onChange={(e) => onModelNameChange(e.target.value)}
              placeholder={modelIdPlaceholder}
              className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
            />
            <p className="text-[11px] text-gray-500">Exact identifier used for API calls</p>
          </>
        )}
      </div>

      {error && (
        <div className="rounded-xl border border-red-400/30 bg-red-400/10 px-4 py-3">
          <p className="text-sm text-red-200">{error}</p>
        </div>
      )}

      <div className="space-y-2 pt-2">
        <button
          onClick={onSave}
          disabled={!canSave || isSaving}
          className="w-full min-h-12 rounded-xl border border-emerald-400/40 bg-emerald-400/20 px-4 py-3 font-semibold text-emerald-100 transition-all duration-200 hover:border-emerald-300/80 hover:bg-emerald-400/30 active:scale-[0.98] disabled:cursor-not-allowed disabled:border-emerald-400/10 disabled:bg-emerald-400/5 disabled:text-emerald-400"
        >
          {isSaving ? (
            <div className="flex items-center justify-center gap-2">
              <Loader size={14} className="animate-spin" />
              Verifying...
            </div>
          ) : (
            "Next: Memory System"
          )}
        </button>

        <button
          onClick={onSkip}
          className="w-full min-h-11 rounded-xl border border-white/10 bg-white/5 px-6 py-3 text-sm font-medium text-gray-400 transition-all duration-200 hover:border-white/20 hover:bg-white/10 hover:text-white active:scale-[0.98]"
        >
          Skip for now
        </button>
      </div>

      {!canSave && (
        <p className="text-xs text-center text-gray-500">
          Fill out both fields above to enable the finish button.
        </p>
      )}
    </div>
  );
}
