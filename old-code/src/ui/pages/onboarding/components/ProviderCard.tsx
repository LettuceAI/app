import { Check } from "lucide-react";
import type { ProviderCapabilitiesCamel } from "../../../../core/providers/capabilities";
import { getProviderIcon } from "../../../../core/utils/providerIcons";
import { useI18n } from "../../../../core/i18n/context";
import type { TranslationKey, TranslateParams } from "../../../../core/i18n/context";

type TFunction = (key: TranslationKey, params?: TranslateParams) => string;

interface ProviderCardProps {
  provider: ProviderCapabilitiesCamel;
  isActive: boolean;
  onClick: () => void;
  variant?: "standard" | "compact";
}

export function ProviderCard({
  provider,
  isActive,
  onClick,
  variant = "standard",
}: ProviderCardProps) {
  const { t } = useI18n();
  if (variant === "compact") {
    return (
      <button
        className={`relative group rounded-xl border px-4 py-3 text-left transition-all duration-200 ${
          isActive
            ? "border-emerald-400/50 bg-emerald-500/20 ring-1 ring-emerald-400/40"
            : "border-white/15 bg-black/35 hover:border-white/25 hover:bg-black/45 active:scale-[0.98]"
        }`}
        onClick={onClick}
        aria-label={`${t("onboarding.steps.provider")}: ${provider.name}`}
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
            <p className="text-[13px] text-white/65 leading-snug truncate">
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

  // variant mobile
  return (
    <button
      className={`relative group min-h-[88px] rounded-2xl border px-3 py-3 text-left transition-all duration-200 ${
        isActive
          ? "border-white/30 bg-black/50 shadow-lg"
          : "border-white/15 bg-black/35 hover:border-white/25 hover:bg-black/45 active:scale-[0.98]"
      }`}
      onClick={onClick}
      aria-label={`${t("onboarding.steps.provider")}: ${provider.name}`}
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
          <p className="text-[12px] text-white/65 leading-snug line-clamp-2">
            {getProviderDescription(provider.id, t)}
          </p>
        </div>
      </div>
    </button>
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
    case "nvidia":
      return t("onboarding.provider.descriptions.nvidia");
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
    case "nvidia":
      return t("onboarding.provider.descriptionsShort.nvidia");
    case "custom":
      return t("onboarding.provider.descriptionsShort.custom");
    default:
      return t("onboarding.provider.descriptionsShort.fallback");
  }
}
