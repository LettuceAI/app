import { useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { ExternalLink, ArrowLeft, ArrowRight, Loader2, Sparkles } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { cn, radius } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { getProviderIcon } from "../../../../core/utils/providerIcons";
import { openExternalUrl } from "../../../../core/utils/openExternal";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { GuidedCarousel, type GuidedSlide } from "./GuidedCarousel";
import { addOrUpdateProviderCredential, addOrUpdateModel } from "../../../../core/storage/repo";
import { createDefaultAdvancedModelSettings } from "../../../../core/storage/schemas";
import {
  setProviderSetupCompleted,
  setModelSetupCompleted,
} from "../../../../core/storage/appState";

import m1a from "../../../../assets/google_gemini_setup_mobile_1.png";
import m1b from "../../../../assets/google_gemini_setup_mobile_1-2.png";
import m2a from "../../../../assets/google_gemini_setup_mobile_2.png";
import m2b from "../../../../assets/google_gemini_setup_mobile_2-2.png";
import m3a from "../../../../assets/google_gemini_setup_mobile_3.png";
import m4a from "../../../../assets/google_gemini_setup_mobile_4.png";
import m4b from "../../../../assets/google_gemini_setup_mobile_4-2.png";
import m5a from "../../../../assets/google_gemini_setup_mobile_5.png";
import m5b from "../../../../assets/google_gemini_setup_mobile_5-1.png";

import d1 from "../../../../assets/google_gemini_setup_desktop_1.png";
import d2 from "../../../../assets/google_gemini_setup_desktop_2.png";
import d3 from "../../../../assets/google_gemini_setup_desktop_3.png";
import d3a from "../../../../assets/google_gemini_setup_desktop_3-1.png";
import d3b from "../../../../assets/google_gemini_setup_desktop_3-2.png";
import d3c from "../../../../assets/google_gemini_setup_desktop_3-3.png";
import d4 from "../../../../assets/google_gemini_setup_desktop_4.png";

const AI_STUDIO_URL = "https://aistudio.google.com/welcome";
const GEMINI_MODEL_ID = "gemma-4-31b-it";
const GEMINI_MODEL_NAME = "Gemma 4 31B";

const stepVariants = {
  enter: (dir: number) => ({ opacity: 0, y: dir * 14 }),
  center: { opacity: 1, y: 0 },
  exit: (dir: number) => ({ opacity: 0, y: dir * -14 }),
};

type GuidedStep = { key: string; slides: GuidedSlide[]; openUrl?: string };

const MOBILE_STEPS: GuidedStep[] = [
  {
    key: "step1",
    openUrl: AI_STUDIO_URL,
    slides: [
      { src: m1a, captionKey: "slide0" },
      { src: m1b, captionKey: "slide1" },
    ],
  },
  {
    key: "step2",
    slides: [
      { src: m2a, captionKey: "slide0" },
      { src: m2b, captionKey: "slide1" },
    ],
  },
  { key: "step3", slides: [{ src: m3a, captionKey: "slide0" }] },
  {
    key: "step4",
    slides: [
      { src: m4a, captionKey: "slide0" },
      { src: m4b, captionKey: "slide1" },
    ],
  },
  {
    key: "step5",
    slides: [
      { src: m5a, captionKey: "slide0" },
      { src: m5b, captionKey: "slide1" },
    ],
  },
  { key: "key", slides: [] },
];

const DESKTOP_STEPS: GuidedStep[] = [
  { key: "step1", openUrl: AI_STUDIO_URL, slides: [{ src: d1, captionKey: "slide0" }] },
  { key: "step2", slides: [{ src: d2, captionKey: "slide0" }] },
  {
    key: "step3",
    slides: [
      { src: d3, captionKey: "slide0" },
      { src: d3a, captionKey: "slide1" },
      { src: d3b, captionKey: "slide2" },
      { src: d3c, captionKey: "slide3" },
    ],
  },
  { key: "step4", slides: [{ src: d4, captionKey: "slide0" }] },
  { key: "key", slides: [] },
];

function ApiKeyStep() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleConnect = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    setError(null);
    try {
      const credentialId = crypto.randomUUID();
      const verification = await invoke<{ valid: boolean; error?: string }>(
        "verify_provider_api_key",
        { providerId: "gemini", credentialId, apiKey: trimmed, baseUrl: null },
      );
      if (!verification.valid) {
        setError(t("onboarding.gemini.key.invalid"));
        setBusy(false);
        return;
      }

      const credential = await addOrUpdateProviderCredential({
        id: credentialId,
        providerId: "gemini",
        label: "Google Gemini",
        apiKey: trimmed,
      });

      await addOrUpdateModel({
        name: GEMINI_MODEL_ID,
        providerId: "gemini",
        providerCredentialId: credential.id,
        providerLabel: credential.label,
        displayName: GEMINI_MODEL_NAME,
        inputScopes: ["text"],
        outputScopes: ["text"],
        advancedModelSettings: {
          ...createDefaultAdvancedModelSettings(),
          reasoningEnabled: true,
        },
      });

      await setProviderSetupCompleted(true);
      await setModelSetupCompleted(true);
      navigate("/onboarding/finish");
    } catch {
      setError(t("onboarding.gemini.key.failed"));
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col">
      <p className="mb-5 text-[14px] leading-relaxed text-white/65">
        {t("onboarding.gemini.key.body")}
      </p>
      <input
        type="text"
        value={apiKey}
        onChange={(e) => {
          setApiKey(e.target.value);
          if (error) setError(null);
        }}
        placeholder={t("onboarding.gemini.key.placeholder")}
        spellCheck={false}
        autoCorrect="off"
        autoCapitalize="off"
        disabled={busy}
        className={cn(
          "w-full border bg-black/60 px-4 py-3 text-[14px] text-white placeholder-white/45 shadow-lg backdrop-blur-md transition focus:outline-none",
          radius.md,
          error ? "border-danger/50 focus:border-danger/70" : "border-white/20 focus:border-white/40",
        )}
      />
      {error && <p className="mt-2 flex items-start gap-1.5 rounded-lg border border-red-400/30 bg-red-950/60 px-3 py-2 text-[12px] leading-relaxed text-red-200 shadow-lg backdrop-blur-md">{error}</p>}

      <button
        type="button"
        onClick={() => void handleConnect()}
        disabled={busy || !apiKey.trim()}
        className={cn(
          "mt-5 flex items-center justify-center gap-2 border border-emerald-400/50 bg-emerald-500/40 shadow-lg backdrop-blur-md py-3 text-[14px] font-semibold text-emerald-50 transition hover:bg-emerald-500/50 active:scale-[0.99]",
          radius.md,
          "disabled:cursor-not-allowed disabled:opacity-50",
        )}
      >
        {busy ? (
          <>
            <Loader2 size={16} className="animate-spin" />
            {t("onboarding.gemini.key.connecting")}
          </>
        ) : (
          <>
            <Sparkles size={16} />
            {t("onboarding.gemini.key.connect")}
          </>
        )}
      </button>
    </div>
  );
}

export function GeminiSetupStep({ onExitBack }: { onExitBack: () => void }) {
  const { t } = useI18n();
  const isDesktop = getPlatform().type === "desktop";
  const variant = isDesktop ? "desktop" : "mobile";
  const STEPS = isDesktop ? DESKTOP_STEPS : MOBILE_STEPS;

  const [stepIndex, setStepIndex] = useState(0);
  const [direction, setDirection] = useState(1);
  const step = STEPS[Math.min(stepIndex, STEPS.length - 1)];
  const isKeyStep = step.key === "key";
  const isLastScreenshot = stepIndex === STEPS.length - 2;

  const goPrev = () => {
    if (stepIndex === 0) {
      onExitBack();
      return;
    }
    setDirection(-1);
    setStepIndex((i) => i - 1);
  };
  const goNext = () => {
    setDirection(1);
    setStepIndex((i) => Math.min(i + 1, STEPS.length - 1));
  };

  const titleKey = isKeyStep
    ? "onboarding.gemini.key.title"
    : `onboarding.gemini.${variant}.${step.key}.title`;
  const bodyKey = isKeyStep
    ? "onboarding.gemini.key.body"
    : `onboarding.gemini.${variant}.${step.key}.body`;

  const lead = (
    <div className="flex items-center gap-2.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-white/10 [&>img]:h-5 [&>img]:w-5 [&>img]:object-contain">
        {getProviderIcon("gemini")}
      </div>
      <span className="text-[11px] font-semibold uppercase tracking-[0.2em] text-emerald-300/90 [text-shadow:0_1px_10px_rgba(0,0,0,0.85)]">
        {t("onboarding.steps.stepNofM", { current: stepIndex + 1, total: STEPS.length })}
      </span>
    </div>
  );

  const title = (
    <h1
      className={cn(
        "mt-3 font-semibold leading-tight text-white [text-shadow:0_1px_12px_rgba(0,0,0,0.9)]",
        "text-[22px] lg:text-[28px]",
      )}
    >
      {t(titleKey as TranslationKey)}
    </h1>
  );

  const progress = (
    <div className="mt-5 flex max-w-xs items-center gap-1.5 rounded-full bg-black/30 p-0.5">
      {STEPS.map((s, i) => (
        <span
          key={s.key}
          className={cn(
            "h-1 flex-1 rounded-full transition-colors",
            i <= stepIndex ? "bg-emerald-400" : "bg-white/15",
          )}
        />
      ))}
    </div>
  );

  const body = (
    <p className="mt-5 text-[15px] leading-relaxed text-white/75 [text-shadow:0_1px_10px_rgba(0,0,0,0.7)]">
      {t(bodyKey as TranslationKey)}
    </p>
  );

  const actions = (
    <div className="mt-7 flex flex-col gap-3">
      {step.openUrl && (
        <button
          type="button"
          onClick={() => void openExternalUrl(step.openUrl!)}
          className={cn(
            "flex items-center justify-center gap-2 border border-white/15 bg-black/55 shadow-lg backdrop-blur-md py-3 text-[14px] font-medium text-white transition hover:border-white/25 hover:bg-black/65 active:scale-[0.99]",
            radius.md,
          )}
        >
          <ExternalLink size={16} />
          {t("onboarding.gemini.openSite")}
        </button>
      )}
      <button
        type="button"
        onClick={goNext}
        className={cn(
          "flex items-center justify-center gap-2 border border-emerald-400/50 bg-emerald-500/40 shadow-lg backdrop-blur-md py-3 text-[14px] font-semibold text-emerald-50 transition hover:bg-emerald-500/50 active:scale-[0.99]",
          radius.md,
        )}
      >
        {isLastScreenshot ? t("onboarding.gemini.key.title") : t("common.buttons.next")}
        <ArrowRight size={16} />
      </button>
    </div>
  );

  const backButton = (
    <button
      type="button"
      onClick={goPrev}
      className="mt-4 flex items-center justify-center gap-1.5 self-center text-[13px] font-medium text-white/70 [text-shadow:0_1px_8px_rgba(0,0,0,0.7)] transition hover:text-white/70"
    >
      <ArrowLeft size={14} />
      {t("common.buttons.back")}
    </button>
  );

  const centered = isDesktop || isKeyStep;
  let containerClass: string;
  let inner: ReactNode;

  if (isKeyStep) {
    containerClass = "mx-auto flex w-full max-w-md flex-1 flex-col px-4";
    inner = (
      <>
        {lead}
        {title}
        {progress}
        <div className="mt-2">
          <ApiKeyStep />
        </div>
        {backButton}
      </>
    );
  } else if (isDesktop) {
    containerClass = "mx-auto flex w-full max-w-7xl flex-1 flex-col px-8";
    inner = (
      <div className="grid items-center gap-14 lg:grid-cols-[minmax(0,320px)_minmax(0,1fr)]">
        <div>
          {lead}
          {title}
          {progress}
          {body}
          {actions}
          <button
            type="button"
            onClick={goPrev}
            className="mt-4 flex items-center gap-1.5 text-[13px] font-medium text-white/70 [text-shadow:0_1px_8px_rgba(0,0,0,0.7)] transition hover:text-white/70"
          >
            <ArrowLeft size={14} />
            {t("common.buttons.back")}
          </button>
        </div>
        <GuidedCarousel
          captionBase={`onboarding.gemini.${variant}.${step.key}`}
          slides={step.slides}
          variant="desktop"
        />
      </div>
    );
  } else {
    containerClass = "mx-auto flex w-full max-w-md flex-1 flex-col";
    inner = (
      <>
        {lead}
        {title}
        {progress}
        {body}
        <div className="mt-6">
          <GuidedCarousel
            captionBase={`onboarding.gemini.${variant}.${step.key}`}
            slides={step.slides}
          />
        </div>
        {actions}
        {backButton}
      </>
    );
  }

  return (
    <div className={containerClass}>
      <AnimatePresence mode="wait" custom={direction} initial={false}>
        <motion.div
          key={stepIndex}
          custom={direction}
          variants={stepVariants}
          initial="enter"
          animate="center"
          exit="exit"
          transition={{ duration: 0.2, ease: "easeOut" }}
          className={cn(
            "flex min-h-full w-full flex-1 flex-col",
            centered && "justify-center",
          )}
        >
          {inner}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}
