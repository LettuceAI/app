import { useMemo } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { ArrowLeft, ExternalLink } from "lucide-react";
import { DISCORD_SERVER_LINK } from "../../../core/utils/links";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { useI18n, type TranslationKey, type TranslateParams } from "../../../core/i18n/context";

type TFunction = (key: TranslationKey, params?: TranslateParams) => string;

type Guide = {
  title: string;
  steps: string[];
  url: string;
};

function buildGuides(t: TFunction): Record<string, Guide> {
  const guideSteps = (provider: string): string[] => [
    t(`onboarding.whereToFind.guides.${provider}.s1` as TranslationKey),
    t(`onboarding.whereToFind.guides.${provider}.s2` as TranslationKey),
    t(`onboarding.whereToFind.guides.${provider}.s3` as TranslationKey),
    t(`onboarding.whereToFind.guides.${provider}.s4` as TranslationKey),
  ];
  const guideTitle = (provider: string): string =>
    t(`onboarding.whereToFind.guides.${provider}.title` as TranslationKey);

  return {
    chutes: {
      title: guideTitle("chutes"),
      steps: guideSteps("chutes"),
      url: "https://chutes.ai/app",
    },
    openai: {
      title: guideTitle("openai"),
      steps: guideSteps("openai"),
      url: "https://platform.openai.com/api-keys",
    },
    cerebras: {
      title: guideTitle("cerebras"),
      steps: guideSteps("cerebras"),
      url: "https://cloud.cerebras.ai/",
    },
    anthropic: {
      title: guideTitle("anthropic"),
      steps: guideSteps("anthropic"),
      url: "https://console.anthropic.com/settings/keys",
    },
    openrouter: {
      title: guideTitle("openrouter"),
      steps: guideSteps("openrouter"),
      url: "https://openrouter.ai/keys",
    },
    mistral: {
      title: guideTitle("mistral"),
      steps: guideSteps("mistral"),
      url: "https://console.mistral.ai/api-keys",
    },
    deepseek: {
      title: guideTitle("deepseek"),
      steps: guideSteps("deepseek"),
      url: "https://platform.deepseek.com/api-keys",
    },
    groq: {
      title: guideTitle("groq"),
      steps: guideSteps("groq"),
      url: "https://console.groq.com/keys",
    },
    gemini: {
      title: guideTitle("gemini"),
      steps: guideSteps("gemini"),
      url: "https://aistudio.google.com/app/apikey",
    },
    xai: {
      title: guideTitle("xai"),
      steps: guideSteps("xai"),
      url: "https://console.x.ai/",
    },
    zai: {
      title: guideTitle("zai"),
      steps: guideSteps("zai"),
      url: "https://open.bigmodel.cn/usercenter/apikeys",
    },
    moonshot: {
      title: guideTitle("moonshot"),
      steps: guideSteps("moonshot"),
      url: "https://platform.moonshot.cn/console/api-keys",
    },
    qwen: {
      title: guideTitle("qwen"),
      steps: guideSteps("qwen"),
      url: "https://dashscope.aliyun.com/apiKey",
    },
    nanogpt: {
      title: guideTitle("nanogpt"),
      steps: guideSteps("nanogpt"),
      url: "https://nano-gpt.com/dashboard/api-keys",
    },
    featherless: {
      title: guideTitle("featherless"),
      steps: guideSteps("featherless"),
      url: "https://featherless.ai/",
    },
    anannas: {
      title: guideTitle("anannas"),
      steps: guideSteps("anannas"),
      url: "https://dashboard.anannas.ai/",
    },
    default: {
      title: guideTitle("default"),
      steps: guideSteps("default"),
      url: "https://platform.openai.com/api-keys",
    },
  };
}

export function WhereToFindPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const providerId = searchParams.get("provider") || "default";

  const guide = useMemo(() => {
    const guides = buildGuides(t);
    return guides[providerId] || guides.default;
  }, [providerId, t]);

  const openLink = (url: string) => openExternalUrl(url);

  return (
    <div className="flex min-h-screen flex-col bg-black text-white px-4 pb-12 pt-[calc(env(safe-area-inset-top)+12px)]">
      <div className="mb-6 flex items-center justify-between">
        <button
          onClick={() => navigate(-1)}
          className="flex items-center justify-center rounded-full border border-white/10 bg-white/10 text-white hover:border-white/25 hover:bg-white/15 active:scale-95 transition"
          aria-label={t("common.buttons.goBack")}
        >
          <ArrowLeft size={10} />
        </button>
        <div className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[12px] uppercase tracking-[0.15em] text-white/60">
          {t("onboarding.whereToFind.badge")}
        </div>
        <div className="w-10" />
      </div>

      <h1 className="text-[25px] font-semibold leading-tight text-white">{guide.title}</h1>
      <p className="mt-2 text-[15px] text-white/65">
        {t("onboarding.whereToFind.intro")}
      </p>

      <div className="mt-6 space-y-3">
        {guide.steps.map((step, index) => (
          <div
            key={index}
            className="flex items-start gap-3 rounded-2xl border border-white/10 bg-white/5 px-4 py-3 hover:border-white/20 hover:bg-white/10 transition"
          >
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-white/10 text-[15px] font-semibold text-white">
              {index + 1}
            </div>
            <p className="text-[15px] text-white/85 leading-relaxed">{step}</p>
          </div>
        ))}
      </div>

      <div className="mt-8 space-y-2">
        <p className="text-[13px] text-white/60">{t("onboarding.whereToFind.readyPrompt")}</p>
        <button
          onClick={() => openLink(guide.url)}
          className="flex w-full items-center justify-center gap-2 rounded-xl border border-white/20 bg-white/10 px-4 py-3 text-[15px] font-semibold text-white transition hover:border-white/30 hover:bg-white/15 active:scale-[0.98]"
        >
          <ExternalLink size={16} />
          {t("onboarding.whereToFind.openProviderSite")}
        </button>
        <p className="text-[12px] text-white/40 mt-1">
          {t("onboarding.whereToFind.keyWarning")}
        </p>
      </div>

      <div className="mt-3 py-4">
        <p className="text-[13px] text-white/60 mb-3">{t("onboarding.whereToFind.stuckPrompt")}</p>
        <button
          onClick={() => openLink(DISCORD_SERVER_LINK)}
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-indigo-600/80 hover:bg-indigo-600 px-4 py-2 text-[15px] font-medium text-white transition active:scale-[0.98]"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <path d="M20.317 4.37a19.791 19.791 0 00-4.885-1.515.074.074 0 00-.079.037c-.211.375-.444.865-.607 1.252a18.27 18.27 0 00-5.487 0c-.163-.387-.399-.877-.609-1.252a.077.077 0 00-.079-.037A19.736 19.736 0 003.677 4.37a.07.07 0 00-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 00.031.056 19.9 19.9 0 005.993 3.03.078.078 0 00.084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 00-.042-.106 13.107 13.107 0 01-1.872-.892.077.077 0 01-.008-.128 10.2 10.2 0 00.372-.294.075.075 0 01.078-.01c3.927 1.793 8.18 1.793 12.062 0a.075.075 0 01.079.009c.12.098.246.198.373.295a.077.077 0 01-.006.127 12.299 12.299 0 01-1.873.892.076.076 0 00-.041.107c.352.699.764 1.365 1.225 1.994a.077.077 0 00.084.028 19.963 19.963 0 006.002-3.03.077.077 0 00.032-.054c.5-4.786-.838-8.95-3.549-12.676a.061.061 0 00-.031-.03zM8.02 15.33c-1.183 0-2.157-.965-2.157-2.156 0-1.193.966-2.157 2.157-2.157 1.193 0 2.169.973 2.157 2.157 0 1.191-.966 2.156-2.157 2.156zm7.975 0c-1.183 0-2.157-.965-2.157-2.156 0-1.193.966-2.157 2.157-2.157 1.193 0 2.169.973 2.157 2.157 0 1.191-.964 2.156-2.157 2.156z" />
          </svg>
          {t("onboarding.whereToFind.joinDiscord")}
        </button>
      </div>
    </div>
  );
}
