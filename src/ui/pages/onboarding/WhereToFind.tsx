import { useMemo } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { ArrowLeft, ExternalLink } from "lucide-react";
import { DISCORD_SERVER_LINK } from "../../../core/utils/links";
import { useI18n } from "../../../core/i18n/context";

type Guide = {
  title: string;
  steps: string[];
  url: string;
};

const guides: Record<string, Guide> = {
  chutes: {
    title: "How to find your Chutes API key",
    steps: [
      "Go to chutes.ai/app and sign in.",
      "Open your account/settings area and find API Keys.",
      "Create a new key (or copy an existing one).",
      "Paste the key into LettuceAI.",
    ],
    url: "https://chutes.ai/app",
  },
  openai: {
    title: "How to find your OpenAI API key",
    steps: [
      "Go to platform.openai.com and sign in.",
      "Click your profile avatar in the top-right, then choose API keys.",
      "Click Create new secret key and copy the value shown.",
      "Paste the key into LettuceAI and store it somewhere safe. You will not see it again.",
    ],
    url: "https://platform.openai.com/api-keys",
  },
  anthropic: {
    title: "How to find your Anthropic API key",
    steps: [
      "Go to console.anthropic.com and sign in.",
      "Open Settings from the left sidebar.",
      "Select API keys and click Create key.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://console.anthropic.com/settings/keys",
  },
  openrouter: {
    title: "How to find your OpenRouter API key",
    steps: [
      "Visit openrouter.ai and log in.",
      "Open the Keys page from your profile menu.",
      "Click Create key, give it a name, and save.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://openrouter.ai/keys",
  },
  mistral: {
    title: "How to find your Mistral API key",
    steps: [
      "Go to console.mistral.ai and sign in.",
      "Click API keys in the sidebar.",
      "Click Create an API key.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://console.mistral.ai/api-keys",
  },
  deepseek: {
    title: "How to find your DeepSeek API key",
    steps: [
      "Open platform.deepseek.com and log in.",
      "Click API Keys in the top navigation.",
      "Create a new key if you do not already have one.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://platform.deepseek.com/api-keys",
  },
  groq: {
    title: "How to find your Groq API key",
    steps: [
      "Visit console.groq.com and sign in.",
      "Open API Keys from the sidebar.",
      "Create a new key, then copy it.",
      "Paste the key into LettuceAI.",
    ],
    url: "https://console.groq.com/keys",
  },
  gemini: {
    title: "How to find your Google Gemini API key",
    steps: [
      "Go to Google AI Studio at aistudio.google.com and sign in.",
      "Click Get API key or Manage keys.",
      "Create a new key if needed.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://aistudio.google.com/app/apikey",
  },
  xai: {
    title: "How to find your xAI API key",
    steps: [
      "Open console.x.ai and sign in.",
      "Navigate to the API Keys section in the console.",
      "Create a new key.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://console.x.ai/",
  },
  zai: {
    title: "How to find your zAI (GLM) API key",
    steps: [
      "Go to open.bigmodel.cn and log in.",
      "Open User Center, then go to API Keys.",
      "Create a new key if you do not have one.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://open.bigmodel.cn/usercenter/apikeys",
  },
  moonshot: {
    title: "How to find your Moonshot (Kimi) API key",
    steps: [
      "Visit platform.moonshot.cn and sign in.",
      "Open the API Keys section in the console.",
      "Create a new key and copy it.",
      "Paste the key into LettuceAI.",
    ],
    url: "https://platform.moonshot.cn/console/api-keys",
  },
  qwen: {
    title: "How to find your Qwen API key",
    steps: [
      "Open dashscope.aliyun.com and log in.",
      "Go to the API Keys section in the sidebar.",
      "Create a new key.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://dashscope.aliyun.com/apiKey",
  },
  nanogpt: {
    title: "How to find your NanoGPT API key",
    steps: [
      "Go to nano-gpt.com and log in.",
      "Open the dashboard and go to the API keys section.",
      "Create a new key if needed.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://nano-gpt.com/dashboard/api-keys",
  },
  featherless: {
    title: "How to find your Featherless API key",
    steps: [
      "Visit featherless.ai and sign in.",
      "Open your account or API section from the dashboard.",
      "Create a new key if you do not see one.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://featherless.ai/",
  },
  anannas: {
    title: "How to find your Anannas API key",
    steps: [
      "Go to dashboard.anannas.ai and log in.",
      "Navigate to the API Keys section.",
      "Create a new key and copy it.",
      "Paste the key into LettuceAI.",
    ],
    url: "https://dashboard.anannas.ai/",
  },
  pollinations: {
    title: "How to find your Pollinations API key",
    steps: [
      "Go to enter.pollinations.ai and sign in.",
      "Navigate to the API section of your dashboard.",
      "Create a new key or copy an existing one.",
      "Paste the key into LettuceAI.",
    ],
    url: "https://enter.pollinations.ai",
  },
  "pollinations-image": {
    title: "How to find your Pollinations API key",
    steps: [
      "Go to enter.pollinations.ai and sign in.",
      "Navigate to the API section of your dashboard.",
      "Create a new key or copy an existing one.",
      "Paste the key into LettuceAI.",
    ],
    url: "https://enter.pollinations.ai",
  },
  automatic1111: {
    title: "How to connect AUTOMATIC1111",
    steps: [
      "Launch your local AUTOMATIC1111 WebUI with the --api command line flag.",
      "No API key is required to connect to a local instance.",
      "Ensure the Base URL points to your instance (usually http://127.0.0.1:7860).",
    ],
    url: "https://github.com/AUTOMATIC1111/stable-diffusion-webui/wiki/API",
  },
  stability: {
    title: "How to find your Stability AI key",
    steps: [
      "Go to platform.stability.ai and sign in.",
      "Navigate to the API Keys page in your account portal.",
      "Create a new key to access the Stable Diffusion APIs.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://platform.stability.ai/keys",
  },
  default: {
    title: "How to find your API key",
    steps: [
      "Open your AI provider dashboard in a browser and sign in.",
      "Look for API, Developer, or Integrations settings.",
      "Create a new API key or view an existing one.",
      "Copy the key and paste it into LettuceAI.",
    ],
    url: "https://platform.openai.com/api-keys",
  },
};

export function WhereToFindPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const providerId = searchParams.get("provider") || "default";

  const guide = useMemo(() => guides[providerId] || guides.default, [providerId]);

  const openLink = async (url: string) => {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } catch (error) {
      console.error("Failed to open URL:", error);
      window.open(url, "_blank");
    }
  };

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
        <div className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[11px] uppercase tracking-[0.15em] text-white/60">
          API Key Help
        </div>
        <div className="w-10" />
      </div>

      <h1 className="text-2xl font-semibold leading-tight text-white">{guide.title}</h1>
      <p className="mt-2 text-sm text-white/65">
        Follow these steps to get your API key, then return to LettuceAI and paste it into the
        provider settings.
      </p>

      <div className="mt-6 space-y-3">
        {guide.steps.map((step, index) => (
          <div
            key={index}
            className="flex items-start gap-3 rounded-2xl border border-white/10 bg-white/5 px-4 py-3 hover:border-white/20 hover:bg-white/10 transition"
          >
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-white/10 text-sm font-semibold text-white">
              {index + 1}
            </div>
            <p className="text-sm text-white/85 leading-relaxed">{step}</p>
          </div>
        ))}
      </div>

      <div className="mt-8 space-y-2">
        <p className="text-xs text-white/60">Ready to get the key?</p>
        <button
          onClick={() => openLink(guide.url)}
          className="flex w-full items-center justify-center gap-2 rounded-xl border border-white/20 bg-white/10 px-4 py-3 text-sm font-semibold text-white transition hover:border-white/30 hover:bg-white/15 active:scale-[0.98]"
        >
          <ExternalLink size={16} />
          Open provider site
        </button>
        <p className="text-[11px] text-white/40 mt-1">
          Never share your API key publicly. Anyone with this key can use your account balance.
        </p>
      </div>

      <div className="mt-3 py-4">
        <p className="text-xs text-white/60 mb-3">Still can't figure it out?</p>
        <button
          onClick={() => openLink(DISCORD_SERVER_LINK)}
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-indigo-600/80 hover:bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition active:scale-[0.98]"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <path d="M20.317 4.37a19.791 19.791 0 00-4.885-1.515.074.074 0 00-.079.037c-.211.375-.444.865-.607 1.252a18.27 18.27 0 00-5.487 0c-.163-.387-.399-.877-.609-1.252a.077.077 0 00-.079-.037A19.736 19.736 0 003.677 4.37a.07.07 0 00-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 00.031.056 19.9 19.9 0 005.993 3.03.078.078 0 00.084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 00-.042-.106 13.107 13.107 0 01-1.872-.892.077.077 0 01-.008-.128 10.2 10.2 0 00.372-.294.075.075 0 01.078-.01c3.927 1.793 8.18 1.793 12.062 0a.075.075 0 01.079.009c.12.098.246.198.373.295a.077.077 0 01-.006.127 12.299 12.299 0 01-1.873.892.076.076 0 00-.041.107c.352.699.764 1.365 1.225 1.994a.077.077 0 00.084.028 19.963 19.963 0 006.002-3.03.077.077 0 00.032-.054c.5-4.786-.838-8.95-3.549-12.676a.061.061 0 00-.031-.03zM8.02 15.33c-1.183 0-2.157-.965-2.157-2.156 0-1.193.966-2.157 2.157-2.157 1.193 0 2.169.973 2.157 2.157 0 1.191-.966 2.156-2.157 2.156zm7.975 0c-1.183 0-2.157-.965-2.157-2.156 0-1.193.966-2.157 2.157-2.157 1.193 0 2.169.973 2.157 2.157 0 1.191-.964 2.156-2.157 2.156z" />
          </svg>
          Join our Discord server for help
        </button>
      </div>
    </div>
  );
}
