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
import { useI18n } from "../../../core/i18n/context";
import type { TranslationKey, TranslateParams } from "../../../core/i18n/context";

type TFunction = (key: TranslationKey, params?: TranslateParams) => string;

// Standard provider card for mobile
interface ProviderCardProps {
  provider: ProviderCapabilitiesCamel;
  isActive: boolean;
  onClick: () => void;
  t: TFunction;
}

function ProviderCard({ provider, isActive, onClick, t }: ProviderCardProps) {
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
          <h3 className="text-[15px] font-semibold text-white leading-tight">{provider.name}</h3>
          <p className="text-[12px] text-white/70 leading-snug line-clamp-2">
            {getProviderDescription(provider.id, t)}
          </p>
        </div>
      </div>
    </button>
  );
}

function ProviderCardCompact({ provider, isActive, onClick, t }: ProviderCardProps) {
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
            className={`text-[15px] font-medium leading-tight truncate ${isActive ? "text-emerald-100" : "text-white"}`}
          >
            {provider.name}
          </h3>
          <p className="text-[13px] text-white/55 leading-snug truncate">
            {getProviderDescriptionShort(provider.id, t)}
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
  const { t } = useI18n();
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
      (isDesktop ? capabilities : capabilities.filter((provider) => provider.id !== "llamacpp"))
        .filter((provider) => provider.id !== "lettuce-engine"),
    [capabilities, isDesktop],
  );
  const selectedProvider = visibleCapabilities.find((p) => p.id === selectedProviderId);
  const isCustomProvider = ["custom", "custom-anthropic"].includes(selectedProviderId);
  const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(selectedProviderId);
  const isHostProvider = selectedProviderId === "lettuce-host";
  const showBaseUrl =
    Boolean(selectedProvider) && (isCustomProvider || isLocalProvider || isHostProvider);

  const configFormContent = (
    <div className="space-y-4">
      <div className="space-y-2">
        <label className="text-[13px] font-medium text-white/70">{t("onboarding.provider.fields.displayLabel")}</label>
        <input
          type="text"
          value={label}
          onChange={(e) => handleLabelChange(e.target.value)}
          onPaste={(e) => {
            e.stopPropagation();
            const pastedText = e.clipboardData.getData("text");
            handleLabelChange(pastedText);
          }}
          placeholder={t("onboarding.provider.fields.displayLabelPlaceholder", {
            name: selectedProvider?.name ?? t("onboarding.provider.fields.defaultLabelFallback"),
          })}
          className="w-full min-h-11 rounded-xl border border-white/15 bg-black/50 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
        />
        <p className="text-[12px] text-white/55">{t("onboarding.provider.fields.displayLabelHint")}</p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="text-[13px] font-medium text-white/70">
            {isLocalProvider
              ? t("onboarding.provider.fields.apiKeyOptional")
              : t("onboarding.provider.fields.apiKey")}
          </label>
          <button
            onClick={() =>
              navigate(`/wheretofind${selectedProviderId ? `?provider=${selectedProviderId}` : ""}`)
            }
            className="text-[12px] text-white/70 hover:text-white transition-colors"
          >
            {t("onboarding.provider.fields.whereToFind")}
          </button>
        </div>
        <input
          type="text"
          value={apiKey}
          onChange={(e) => handleApiKeyChange(e.target.value)}
          placeholder={
            isLocalProvider
              ? t("onboarding.provider.fields.apiKeyPlaceholderLocal")
              : t("onboarding.provider.fields.apiKeyPlaceholderRemote")
          }
          className="w-full min-h-11 rounded-xl border border-white/15 bg-black/50 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
        />
        <p className="text-[12px] text-white/55">{t("onboarding.provider.fields.apiKeyHint")}</p>
      </div>

      {showBaseUrl && (
        <div className="space-y-2">
          <label className="text-[13px] font-medium text-white/70">{t("onboarding.provider.fields.baseUrl")}</label>
          <input
            type="text"
            value={baseUrl}
            onChange={(e) => handleBaseUrlChange(e.target.value)}
            onPaste={(e) => {
              e.stopPropagation();
              const pastedText = e.clipboardData.getData("text");
              handleBaseUrlChange(pastedText);
            }}
            placeholder={
              selectedProviderId === "intenserp"
                ? t("onboarding.provider.fields.baseUrlPlaceholderIntenseRP")
                : isHostProvider
                  ? t("onboarding.provider.fields.baseUrlPlaceholderHost")
                  : isLocalProvider
                    ? t("onboarding.provider.fields.baseUrlPlaceholderLocal")
                    : t("onboarding.provider.fields.baseUrlPlaceholderRemote")
            }
            className="w-full min-h-11 rounded-xl border border-white/15 bg-black/50 px-3 py-2 text-white placeholder-white/40 transition-colors focus:border-white/30 focus:outline-none"
          />
          <p className="text-[12px] text-white/55">
            {isLocalProvider
              ? t("onboarding.provider.fields.baseUrlHintLocal")
              : isHostProvider
                ? t("onboarding.provider.fields.baseUrlHintHost")
                : t("onboarding.provider.fields.baseUrlHintRemote")}
          </p>
        </div>
      )}

      {testResult && (
        <div
          className={`rounded-xl border px-4 py-3 text-[15px] ${
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
          className="w-full min-h-11 rounded-xl border border-white/25 bg-white/20 px-4 py-3 font-medium text-white transition-all duration-200 hover:border-white/35 hover:bg-white/25 active:scale-[0.98] disabled:cursor-not-allowed disabled:border-white/10 disabled:bg-white/10 disabled:text-white/55"
        >
          {isTesting ? (
            <div className="flex items-center justify-center gap-2">
              <Loader size={14} className="animate-spin" />
              {t("onboarding.provider.buttons.testing")}
            </div>
          ) : (
            <div className="flex items-center justify-center gap-2">
              <AlertCircle size={14} />
              {t("onboarding.provider.buttons.testConnection")}
            </div>
          )}
        </button>

        <button
          onClick={handleSaveProvider}
          disabled={!canSave || isSubmitting}
          className="w-full min-h-12 rounded-xl border border-emerald-400/60 bg-emerald-500/40 px-4 py-3 font-semibold text-emerald-50 transition-all duration-200 hover:border-emerald-300/90 hover:bg-emerald-500/50 active:scale-[0.98] disabled:cursor-not-allowed disabled:border-emerald-400/20 disabled:bg-emerald-400/10 disabled:text-emerald-400"
        >
          {isSubmitting ? (
            <div className="flex items-center justify-center gap-2">
              <Loader size={14} className="animate-spin" />
              {t("onboarding.common.verifying")}
            </div>
          ) : (
            t("onboarding.common.continue")
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
            <p className="text-[11px] font-medium uppercase tracking-[0.25em] text-white/55">
              {t("onboarding.stepIndicator", { current: 1, total: 3 })}
            </p>
            <p className="text-[13px] text-white/70 mt-0.5">{t("onboarding.steps.provider")}</p>
          </div>
          <div className="w-10" />
        </div>

        {/* Main Content */}
        <div className="flex flex-1 min-h-0">
          {/* Left Panel */}
          <div className="flex-1 flex flex-col border-r border-white/10">
            <div className="p-6 pb-3">
              <h2 className="text-[15px] font-medium text-white/70">{t("onboarding.provider.availableProviders")}</h2>
              <p className="text-[13px] text-white/55 mt-0.5">{t("onboarding.common.clickToSelectProvider")}</p>
            </div>
            <div className="flex-1 overflow-y-auto px-6 pb-10">
              <div className="grid grid-cols-2 xl:grid-cols-3 gap-2">
                {visibleCapabilities.map((provider) => (
                  <ProviderCardCompact
                    key={provider.id}
                    provider={provider}
                    isActive={selectedProviderId === provider.id}
                    t={t}
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
              <h1 className="text-[21px] font-bold text-white">
                {selectedProvider
                  ? t("onboarding.provider.configureProvider", { name: selectedProvider.name })
                  : t("onboarding.provider.chooseProvider")}
              </h1>
              <p className="text-[15px] text-white/70 leading-relaxed">
                {selectedProvider
                  ? t("onboarding.common.enterApiKey")
                  : t("onboarding.common.selectProviderFromList")}
              </p>
            </div>

            {showForm && selectedProvider ? (
              configFormContent
            ) : (
              <div className="rounded-xl border border-dashed border-white/20 bg-white/5 p-6 text-center">
                <p className="text-[15px] text-white/55">{t("onboarding.common.selectAProvider")}</p>
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
            <p className="text-[11px] font-medium uppercase tracking-[0.25em] text-white/55">
              {t("onboarding.stepIndicator", { current: 1, total: 3 })}
            </p>
            <p className="text-[13px] text-white/70 mt-0.5">{t("onboarding.steps.provider")}</p>
          </div>
          <div className="w-10" />
        </div>

        {/* Title */}
        <div className="text-center space-y-2 mb-8">
          <h1 className="text-[25px] font-bold text-white">{t("onboarding.steps.provider")}</h1>
          <p className="text-[15px] text-white/70 max-w-sm leading-relaxed">
            {t("onboarding.provider.descMobile")}
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
                t={t}
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
            <h2 className="text-[19px] font-semibold text-white">{t("onboarding.provider.connectProvider", { name: selectedProvider?.name ?? "" })}</h2>
            <p className="text-[13px] text-white/70 leading-relaxed">
              {t("onboarding.provider.connectProviderDesc")}
            </p>
          </div>

          {selectedProvider ? configFormContent : null}
        </div>
      </div>
      <div id="provider-config-form"></div>
    </div>
  );
}

function getProviderDescription(providerId: string, t: TFunction): string {
  switch (providerId) {
    case "chutes":
      return t("onboarding.provider.descriptions.chutes");
    case "openai":
      return t("onboarding.provider.descriptions.openai");
    case "cerebras":
      return t("onboarding.provider.descriptions.cerebras");
    case "lettuce-host":
      return t("onboarding.provider.descriptions.lettuceHost");
    case "anthropic":
      return t("onboarding.provider.descriptions.anthropic");
    case "nanogpt":
    case "featherless":
    case "openrouter":
      return t("onboarding.provider.descriptions.aggregator");
    case "openai-compatible":
      return t("onboarding.provider.descriptions.openaiCompatible");
    case "mistral":
      return t("onboarding.provider.descriptions.mistral");
    case "deepseek":
      return t("onboarding.provider.descriptions.deepseek");
    case "xai":
      return t("onboarding.provider.descriptions.xai");
    case "zai":
      return t("onboarding.provider.descriptions.zai");
    case "moonshot":
      return t("onboarding.provider.descriptions.moonshot");
    case "gemini":
      return t("onboarding.provider.descriptions.gemini");
    case "qwen":
      return t("onboarding.provider.descriptions.qwen");
    case "custom":
      return t("onboarding.provider.descriptions.custom");
    default:
      return t("onboarding.provider.descriptions.fallback");
  }
}

function getProviderDescriptionShort(providerId: string, t: TFunction): string {
  switch (providerId) {
    case "chutes":
      return t("onboarding.provider.descriptionsShort.chutes");
    case "openai":
      return t("onboarding.provider.descriptionsShort.openai");
    case "cerebras":
      return t("onboarding.provider.descriptionsShort.cerebras");
    case "lettuce-host":
      return t("onboarding.provider.descriptionsShort.lettuceHost");
    case "anthropic":
      return t("onboarding.provider.descriptionsShort.anthropic");
    case "nanogpt":
    case "featherless":
    case "openrouter":
      return t("onboarding.provider.descriptionsShort.aggregator");
    case "openai-compatible":
      return t("onboarding.provider.descriptionsShort.openaiCompatible");
    case "mistral":
      return t("onboarding.provider.descriptionsShort.mistral");
    case "deepseek":
      return t("onboarding.provider.descriptionsShort.deepseek");
    case "xai":
      return t("onboarding.provider.descriptionsShort.xai");
    case "zai":
      return t("onboarding.provider.descriptionsShort.zai");
    case "moonshot":
      return t("onboarding.provider.descriptionsShort.moonshot");
    case "gemini":
      return t("onboarding.provider.descriptionsShort.gemini");
    case "qwen":
      return t("onboarding.provider.descriptionsShort.qwen");
    case "custom":
      return t("onboarding.provider.descriptionsShort.custom");
    default:
      return t("onboarding.provider.descriptionsShort.fallback");
  }
}
