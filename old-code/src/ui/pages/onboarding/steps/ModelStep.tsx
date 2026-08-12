import { useEffect } from "react";
import { Settings, Check, Loader } from "lucide-react";
import type { ProviderCredential } from "../../../../core/storage/schemas";
import { ModelConfigForm } from "../components/ConfigForm";
import { getPlatform } from "../../../../core/utils/platform";
import { getProviderIcon } from "../../../../core/utils/providerIcons";
import { useI18n } from "../../../../core/i18n/context";
import type { TranslationKey, TranslateParams } from "../../../../core/i18n/context";

type TFunction = (key: TranslationKey, params?: TranslateParams) => string;

interface ModelStepProps {
  providers: ProviderCredential[];
  selectedCredential: ProviderCredential | null;
  modelName: string;
  displayName: string;
  error: string | null;
  isLoading: boolean;
  isSaving: boolean;
  canSave: boolean;
  onSelectCredential: (credential: ProviderCredential) => void;
  onModelNameChange: (value: string) => void;
  onDisplayNameChange: (value: string) => void;
  onSave: () => void;
  onSkip: () => void;
  onGoBack: () => void;
}

export function ModelStep({
  providers,
  selectedCredential,
  modelName,
  displayName,
  error,
  isLoading,
  isSaving,
  canSave,
  onSelectCredential,
  onModelNameChange,
  onDisplayNameChange,
  onSave,
  onSkip,
  onGoBack,
}: ModelStepProps) {
  const { t } = useI18n();
  const platform = getPlatform();
  const isDesktop = platform.type === "desktop";
  const showForm = Boolean(selectedCredential);

  useEffect(() => {
    if (isDesktop || !showForm) return;

    const timeout = window.setTimeout(() => {
      const formElement = document.getElementById("model-config-form");
      if (formElement) {
        formElement.scrollIntoView({
          behavior: "smooth",
          block: "end",
          inline: "nearest",
        });
      }
    }, 350);

    return () => window.clearTimeout(timeout);
  }, [showForm, isDesktop]);

  if (isLoading) {
    return (
      <div className="flex flex-1 items-center justify-center text-white/80">
        <div className="flex items-center gap-3 text-[15px]">
          <Loader size={20} className="animate-spin" />
          {t("onboarding.loading")}
        </div>
      </div>
    );
  }

  if (providers.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-6 text-center text-white/80">
        <div className="flex h-16 w-16 items-center justify-center rounded-3xl border border-white/10 bg-white/10 text-gray-200">
          <Settings size={26} />
        </div>
        <div className="space-y-2">
          <h2 className="text-[21px] font-semibold text-white">
            {t("onboarding.model.noProvidersTitle")}
          </h2>
          <p className="max-w-md text-[15px] text-white/70">{t("onboarding.model.noProvidersDesc")}</p>
        </div>
        <button
          onClick={onGoBack}
          className="rounded-full border border-white/15 bg-white/10 px-6 py-3 text-[15px] font-semibold text-white transition hover:border-white/30 hover:bg-white/20"
        >
          {t("onboarding.model.goToProviderSetup")}
        </button>
      </div>
    );
  }

  // Desktop Layout
  if (isDesktop) {
    return (
      <div className="flex flex-1 min-h-0">
        {/* Left Panel - Provider List */}
        <div className="flex-1 flex flex-col border-r border-white/10">
          <div className="p-6 pb-3">
            <h2 className="text-[15px] font-medium text-white/70">{t("onboarding.model.yourProviders")}</h2>
            <p className="text-[13px] text-white/55 mt-0.5">{t("onboarding.model.yourProvidersHint")}</p>
          </div>
          <div className="flex-1 overflow-y-auto px-6">
            <div className="space-y-2">
              {providers.map((provider) => {
                const isActive = selectedCredential?.id === provider.id;
                return (
                  <button
                    key={provider.id}
                    className={`w-full rounded-xl border px-4 py-3 text-left transition-all duration-200 ${
                      isActive
                        ? "border-emerald-400/50 bg-emerald-500/20 ring-1 ring-emerald-400/40"
                        : "border-white/15 bg-black/35 hover:border-white/25 hover:bg-black/45 active:scale-[0.98]"
                    }`}
                    onClick={() => onSelectCredential(provider)}
                  >
                    <div className="flex items-center gap-3">
                      <div
                        className={`flex h-10 w-10 items-center justify-center rounded-lg border ${
                          isActive
                            ? "border-emerald-400/30 bg-emerald-400/10"
                            : "border-white/15 bg-white/8"
                        }`}
                      >
                        {getProviderIcon(provider.providerId)}
                      </div>
                      <div className="flex-1 min-w-0">
                        <h3
                          className={`text-[15px] font-medium ${isActive ? "text-emerald-100" : "text-white"}`}
                        >
                          {provider.label}
                        </h3>
                        <p className="text-[13px] text-white/55 truncate">
                          {getProviderDisplayName(provider.providerId, t)}
                        </p>
                      </div>
                      <div
                        className={`flex h-5 w-5 items-center justify-center rounded-full border transition-colors ${
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
              })}
            </div>
          </div>
        </div>

        {/* Right Panel - Config */}
        <div className="w-100 shrink-0 p-8 overflow-y-auto">
          <div className="space-y-1 mb-6">
            <h1 className="text-[21px] font-bold text-white">
              {selectedCredential
                ? t("onboarding.model.modelDetails")
                : t("onboarding.model.setDefaultModel")}
            </h1>
            <p className="text-[15px] text-white/70 leading-relaxed">
              {selectedCredential
                ? t("onboarding.model.modelDetailsDesc")
                : t("onboarding.model.setDefaultModelDescDesktop")}
            </p>
          </div>

          {selectedCredential ? (
            <ModelConfigForm
              selectedCredential={selectedCredential}
              displayName={displayName}
              modelName={modelName}
              error={error}
              isSaving={isSaving}
              canSave={canSave}
              onDisplayNameChange={onDisplayNameChange}
              onModelNameChange={onModelNameChange}
              onSave={onSave}
              onSkip={onSkip}
            />
          ) : (
            <div className="rounded-xl border border-dashed border-white/20 bg-white/5 p-6 text-center">
              <p className="text-[15px] text-white/55">{t("onboarding.common.selectAProvider")}</p>
            </div>
          )}
        </div>
      </div>
    );
  }

  // Mobile Layout
  return (
    <div className="flex flex-col items-center pb-8">
      {/* Title */}
      <div className="text-center space-y-2 mb-6">
        <h1 className="text-[25px] font-bold text-white">{t("onboarding.model.setDefaultModel")}</h1>
        <p className="text-[15px] text-white/70 max-w-sm leading-relaxed">
          {t("onboarding.model.setDefaultModelDesc")}
        </p>
      </div>

      {/* Provider Selection */}
      <div className="w-full max-w-sm space-y-3 mb-8">
        {providers.map((provider) => {
          const isActive = selectedCredential?.id === provider.id;
          return (
            <button
              key={provider.id}
              className={`w-full min-h-15 rounded-2xl border px-4 py-3 text-left transition-all duration-200 ${
                isActive
                  ? "border-white/30 bg-black/50 shadow-lg"
                  : "border-white/15 bg-black/35 hover:border-white/25 hover:bg-black/45 active:scale-[0.98]"
              }`}
              onClick={() => onSelectCredential(provider)}
            >
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/15 bg-white/8">
                  {getProviderIcon(provider.providerId)}
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="text-[15px] font-semibold text-white">{provider.label}</h3>
                  <p className="text-[13px] text-white/70 truncate">
                    {getProviderDisplayName(provider.providerId, t)}
                  </p>
                </div>
                <div
                  className={`flex h-5 w-5 items-center justify-center rounded-full border transition-colors ${
                    isActive
                      ? "border-emerald-400/60 bg-emerald-400/20 text-emerald-300"
                      : "border-white/20 text-transparent"
                  }`}
                >
                  <Check size={12} />
                </div>
              </div>
            </button>
          );
        })}
      </div>

      {/* Model Configuration */}
      <div
        id="model-config-form"
        className={`w-full max-w-sm transition-all duration-300 ${showForm ? "opacity-100" : "opacity-0 pointer-events-none"}`}
      >
        <div className="text-center space-y-2 mb-6">
          <h2 className="text-[19px] font-semibold text-white">{t("onboarding.model.modelDetails")}</h2>
          <p className="text-[13px] text-white/70 leading-relaxed">
            {t("onboarding.model.modelDetailsDesc")}
          </p>
        </div>

        {selectedCredential && (
          <ModelConfigForm
            selectedCredential={selectedCredential}
            displayName={displayName}
            modelName={modelName}
            error={error}
            isSaving={isSaving}
            canSave={canSave}
            onDisplayNameChange={onDisplayNameChange}
            onModelNameChange={onModelNameChange}
            onSave={onSave}
            onSkip={onSkip}
          />
        )}
      </div>
    </div>
  );
}

function getProviderDisplayName(providerId: string, t: TFunction): string {
  switch (providerId) {
    case "chutes":
      return t("onboarding.model.providerNames.chutes");
    case "openai":
      return t("onboarding.model.providerNames.openai");
    case "anthropic":
      return t("onboarding.model.providerNames.anthropic");
    case "openrouter":
      return t("onboarding.model.providerNames.openrouter");
    case "openai-compatible":
      return t("onboarding.model.providerNames.openaiCompatible");
    case "custom":
      return t("onboarding.model.providerNames.custom");
    default:
      return providerId;
  }
}
