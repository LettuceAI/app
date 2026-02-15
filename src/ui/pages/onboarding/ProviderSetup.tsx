import React from "react";
import { AlertCircle, ArrowLeft, Check, Loader } from "lucide-react";
import { useNavigate } from "react-router-dom";

import {
  getProviderCapabilities,
  toCamel,
  type ProviderCapabilitiesCamel,
} from "../../../core/providers/capabilities";
import { useProviderController } from "./hooks/useProviderController";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { getPlatform } from "../../../core/utils/platform";

// Standard provider card for mobile
interface ProviderCardProps {
  provider: ProviderCapabilitiesCamel;
  isActive: boolean;
  onClick: () => void;
}

function ProviderCard({ provider, isActive, onClick }: ProviderCardProps) {
  return (
    <button
      className={`relative group min-h-22 rounded-2xl border px-3 py-3 text-left transition-all duration-200 ${
        isActive
          ? "border-white/25 bg-white/15 shadow-lg"
          : "border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/10 active:scale-[0.98]"
      }`}
      onClick={onClick}
    >
      <div className="flex flex-col gap-2">
        <div className="flex items-start justify-between gap-2">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-white/15 bg-white/8">
            {getProviderIcon(provider.id)}
          </div>
          <div
            className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors ${
              isActive
                ? "border-emerald-400/60 bg-emerald-400/20 text-emerald-300"
                : "border-white/20 text-transparent"
            }`}
          >
            <Check size={10} />
          </div>
        </div>
        <div className="space-y-0.5">
          <h3 className="text-sm font-semibold text-white leading-tight">{provider.name}</h3>
          <p className="text-[11px] text-gray-400 leading-snug line-clamp-2">
            {getProviderDescription(provider.id)}
          </p>
        </div>
      </div>
    </button>
  );
}

function ProviderCardCompact({ provider, isActive, onClick }: ProviderCardProps) {
  return (
    <button
      className={`relative group rounded-xl border px-4 py-3 text-left transition-all duration-200 ${
        isActive
          ? "border-emerald-400/40 bg-emerald-400/10 ring-1 ring-emerald-400/30"
          : "border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/8 active:scale-[0.98]"
      }`}
      onClick={onClick}
    >
      <div className="flex items-center gap-3">
        <div
          className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border ${
            isActive ? "border-emerald-400/30 bg-emerald-400/10" : "border-white/15 bg-white/8"
          }`}
        >
          {getProviderIcon(provider.id)}
        </div>
        <div className="flex-1 min-w-0">
          <h3
            className={`text-sm font-medium leading-tight truncate ${isActive ? "text-emerald-100" : "text-white"}`}
          >
            {provider.name}
          </h3>
          <p className="text-xs text-gray-500 leading-snug truncate">
            {getProviderDescriptionShort(provider.id)}
          </p>
        </div>
        <div
          className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors ${
            isActive
              ? "border-emerald-400/60 bg-emerald-400/30 text-emerald-200"
              : "border-white/15 text-transparent"
          }`}
        >
          <Check size={10} />
        </div>
      </div>
    </button>
  );
}

export function ProviderSetupPage() {
  const navigate = useNavigate();
  const platform = getPlatform();
  const isDesktop = platform.type === "desktop";

  const {
    state: {
      selectedProviderId,
      label,
      apiKey,
      baseUrl,
      isTesting,
      testResult,
      isSubmitting,
      showForm,
    },
    canTest,
    canSave,
    handleSelectProvider,
    handleLabelChange,
    handleApiKeyChange,
    handleBaseUrlChange,
    handleTestConnection,
    handleSaveProvider,
    goToWelcome,
  } = useProviderController();

  const [capabilities, setCapabilities] = React.useState<ProviderCapabilitiesCamel[]>([]);

  React.useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const caps = (await getProviderCapabilities()).map(toCamel);
        if (!cancelled) setCapabilities(caps);
      } catch (e) {
        console.warn("[Onboarding] Failed to load provider capabilities", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const visibleCapabilities = React.useMemo(
    () =>
      isDesktop ? capabilities : capabilities.filter((provider) => provider.id !== "llamacpp"),
    [capabilities, isDesktop],
  );
  const selectedProvider = visibleCapabilities.find((p) => p.id === selectedProviderId);
  const isCustomProvider = ["custom", "custom-anthropic"].includes(selectedProviderId);
  const isLocalProvider = ["ollama", "lmstudio"].includes(selectedProviderId);
  const showBaseUrl = Boolean(selectedProvider) && (isCustomProvider || isLocalProvider);

  const configFormContent = (
    <div className="space-y-4">
      <div className="space-y-2">
        <label className="text-xs font-medium text-white/70">Display Label</label>
        <input
          type="text"
          value={label}
          onChange={(e) => handleLabelChange(e.target.value)}
          onPaste={(e) => {
            e.stopPropagation();
            const pastedText = e.clipboardData.getData("text");
            handleLabelChange(pastedText);
          }}
          placeholder={`My ${selectedProvider?.name}`}
          className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
        />
        <p className="text-[11px] text-gray-500">How this provider will appear in your menus</p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="text-xs font-medium text-white/70">
            API Key{isLocalProvider ? " (Optional)" : ""}
          </label>
          <button
            onClick={() =>
              navigate(`/wheretofind${selectedProviderId ? `?provider=${selectedProviderId}` : ""}`)
            }
            className="text-[11px] text-gray-400 hover:text-white transition-colors"
          >
            Where to find it
          </button>
        </div>
        <input
          type="text"
          value={apiKey}
          onChange={(e) => handleApiKeyChange(e.target.value)}
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
            onChange={(e) => handleBaseUrlChange(e.target.value)}
            onPaste={(e) => {
              e.stopPropagation();
              const pastedText = e.clipboardData.getData("text");
              handleBaseUrlChange(pastedText);
            }}
            placeholder={isLocalProvider ? "http://localhost:11434" : "https://api.provider.com"}
            className="w-full min-h-11 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
          />
          <p className="text-[11px] text-gray-500">
            {isLocalProvider
              ? "Your local server address with port"
              : "Override the default endpoint if needed"}
          </p>
        </div>
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
          onClick={handleTestConnection}
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
          onClick={handleSaveProvider}
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

  if (isDesktop) {
    return (
      <div className="flex min-h-screen flex-col text-gray-200 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-8 py-6 border-b border-white/10">
          <button
            onClick={goToWelcome}
            className="flex items-center justify-center w-11 h-11 rounded-full border border-white/15 bg-white/8 text-white transition-all duration-200 hover:border-white/30 hover:bg-white/15 active:scale-[0.98]"
          >
            <ArrowLeft size={18} />
          </button>
          <div className="text-center">
            <p className="text-[10px] font-medium uppercase tracking-[0.25em] text-gray-500">
              Step 1 of 3
            </p>
            <p className="text-xs text-gray-400 mt-0.5">Provider Setup</p>
          </div>
          <div className="w-10" />
        </div>

        {/* Main Content */}
        <div className="flex flex-1 min-h-0">
          {/* Left Panel */}
          <div className="flex-1 flex flex-col border-r border-white/10">
            <div className="p-6 pb-3">
              <h2 className="text-sm font-medium text-white/70">Available Providers</h2>
              <p className="text-xs text-gray-500 mt-0.5">Click to select a provider</p>
            </div>
            <div className="flex-1 overflow-y-auto px-6">
              <div className="grid grid-cols-2 xl:grid-cols-3 gap-2">
                {visibleCapabilities.map((provider) => (
                  <ProviderCardCompact
                    key={provider.id}
                    provider={provider}
                    isActive={selectedProviderId === provider.id}
                    onClick={() =>
                      handleSelectProvider({
                        id: provider.id,
                        name: provider.name,
                        defaultBaseUrl: provider.defaultBaseUrl,
                      })
                    }
                  />
                ))}
              </div>
            </div>
          </div>

          {/* Right Panel */}
          <div className="w-100 shrink-0 p-8 overflow-y-auto">
            <div className="space-y-1 mb-6">
              <h1 className="text-xl font-bold text-white">
                {selectedProvider ? `Configure ${selectedProvider.name}` : "Choose a provider"}
              </h1>
              <p className="text-sm text-gray-400 leading-relaxed">
                {selectedProvider
                  ? "Enter your API key to enable AI chat functionality."
                  : "Select a provider from the list to get started."}
              </p>
            </div>

            {showForm && selectedProvider ? (
              configFormContent
            ) : (
              <div className="rounded-xl border border-dashed border-white/20 bg-white/5 p-6 text-center">
                <p className="text-sm text-gray-500">Select a provider to configure</p>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  }

  // Mobile Layout (original)
  return (
    <div className="flex min-h-screen flex-col text-gray-200 px-4 pt-8 overflow-y-auto">
      <div className="flex flex-col items-center pb-8">
        {/* Header */}
        <div className="flex w-full max-w-sm items-center justify-between mb-8">
          <button
            onClick={goToWelcome}
            className="flex items-center justify-center rounded-full border border-white/15 bg-white/8 text-white transition-all duration-200 hover:border-white/30 hover:bg-white/15 active:scale-[0.98]"
          >
            <ArrowLeft size={16} />
          </button>
          <div className="text-center">
            <p className="text-[10px] font-medium uppercase tracking-[0.25em] text-gray-500">
              Step 1 of 3
            </p>
            <p className="text-xs text-gray-400 mt-0.5">Provider Setup</p>
          </div>
          <div className="w-10" />
        </div>

        {/* Title */}
        <div className="text-center space-y-2 mb-8">
          <h1 className="text-2xl font-bold text-white">Choose your AI provider</h1>
          <p className="text-sm text-gray-400 max-w-sm leading-relaxed">
            Select an AI provider to get started. Your API keys are securely encrypted on your
            device. No account signup needed.
          </p>
        </div>

        {/* Provider Selection */}
        <div className="w-full max-w-2xl mb-8">
          <div className="grid grid-cols-2 gap-3">
            {visibleCapabilities.map((provider) => (
              <ProviderCard
                key={provider.id}
                provider={provider}
                isActive={selectedProviderId === provider.id}
                onClick={() =>
                  handleSelectProvider({
                    id: provider.id,
                    name: provider.name,
                    defaultBaseUrl: provider.defaultBaseUrl,
                  })
                }
              />
            ))}
          </div>
        </div>

        {/* Configuration Form */}
        <div
          className={`config-form-section w-full max-w-sm transition-all duration-300 ${showForm && selectedProvider ? "opacity-100 max-h-500" : "opacity-0 max-h-0 overflow-hidden pointer-events-none"}`}
        >
          <div className="text-center space-y-2 mb-6">
            <h2 className="text-lg font-semibold text-white">Connect {selectedProvider?.name}</h2>
            <p className="text-xs text-gray-400 leading-relaxed">
              Paste your API key below to enable chats. Need a key? Get one from the provider
              dashboard.
            </p>
          </div>

          {selectedProvider ? configFormContent : null}
        </div>
      </div>
      <div id="provider-config-form"></div>
    </div>
  );
}

function getProviderDescription(providerId: string): string {
  switch (providerId) {
    case "chutes":
      return "OpenAI-compatible inference for top open-source models";
    case "openai":
      return "GPT-5, GPT-4.1, and GPT-4o models for expressive RP";
    case "anthropic":
      return "Claude 4.5 Sonnet & Haiku for deep, emotional dialogue";
    case "nanogpt":
    case "featherless":
    case "openrouter":
      return "Access models like GPT-5, Claude 4.5, Grok-3, Mixtral, and more";
    case "openai-compatible":
      return "Use any OpenAI-style API endpoint";
    case "mistral":
      return "Mistral Small 3.2, Mixtral 8x22B, and other Mistral models";
    case "deepseek":
      return "DeepSeek-V3.2-Exp, DeepSeek-R1, and other high-efficiency models";
    case "xai":
      return "Grok-1.5, Grok-3, and newer xAI models";
    case "zai":
      return "GLM-4.5, GLM-4.6, and Air variants";
    case "moonshot":
      return "Kimi-K2 Thinking and Kimi-K1 models";
    case "gemini":
      return "Gemini 2.5 Flash, 2.5 Pro, and more";
    case "qwen":
      return "Qwen3-VL and newer Qwen models";
    case "custom":
      return "Point LettuceAI to any custom model endpoint";
    default:
      return "AI model provider";
  }
}

function getProviderDescriptionShort(providerId: string): string {
  switch (providerId) {
    case "chutes":
      return "Open-source model inference";
    case "openai":
      return "GPT-5, GPT-4o, GPT-4.1";
    case "anthropic":
      return "Claude 4.5 Sonnet & Haiku";
    case "nanogpt":
    case "featherless":
    case "openrouter":
      return "Multi-model aggregator";
    case "openai-compatible":
      return "Custom OpenAI endpoint";
    case "mistral":
      return "Mistral & Mixtral models";
    case "deepseek":
      return "DeepSeek-V3, R1";
    case "xai":
      return "Grok-1.5, Grok-3";
    case "zai":
      return "GLM-4.5, GLM-4.6";
    case "moonshot":
      return "Kimi-K2 Thinking";
    case "gemini":
      return "Gemini 2.5 Flash & Pro";
    case "qwen":
      return "Qwen3-VL models";
    case "custom":
      return "Custom endpoint";
    default:
      return "AI provider";
  }
}
