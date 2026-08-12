import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { ExternalLink, ArrowLeft, ArrowRight, Loader2, Check } from "lucide-react";
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

import m1a from "../../../../assets/openrouter_setup_mobile_1.png";
import m2a from "../../../../assets/openrouter_setup_mobile_2.png";
import m2b from "../../../../assets/openrouter_setup_mobile_2-1.png";
import m3a from "../../../../assets/openrouter_setup_mobile_3.png";
import m3b from "../../../../assets/openrouter_setup_mobile_3-1.png";
import m4a from "../../../../assets/openrouter_setup_mobile_4.png";
import m4b from "../../../../assets/openrouter_setup_mobile_4-1.png";

import d1a from "../../../../assets/openrouter_setup_desktop_1.png";
import d2a from "../../../../assets/openrouter_setup_desktop_2.png";
import d2b from "../../../../assets/openrouter_setup_desktop_2-1.png";
import d3a from "../../../../assets/openrouter_setup_desktop_3.png";
import d3b from "../../../../assets/openrouter_setup_desktop_3-1.png";
import d4a from "../../../../assets/openrouter_setup_desktop_4.png";
import d4b from "../../../../assets/openrouter_setup_desktop_4-1.png";

const OPENROUTER_URL = "https://openrouter.ai";

const stepVariants = {
  enter: (dir: number) => ({ opacity: 0, y: dir * 14 }),
  center: { opacity: 1, y: 0 },
  exit: (dir: number) => ({ opacity: 0, y: dir * -14 }),
};


type GuidedStep = { key: string; slides: GuidedSlide[]; openUrl?: string };

const MOBILE_STEPS: GuidedStep[] = [
  { key: "step1", openUrl: OPENROUTER_URL, slides: [{ src: m1a, captionKey: "slide0" }] },
  {
    key: "step2",
    slides: [
      { src: m2a, captionKey: "slide0" },
      { src: m2b, captionKey: "slide1" },
    ],
  },
  {
    key: "step3",
    slides: [
      { src: m3a, captionKey: "slide0" },
      { src: m3b, captionKey: "slide1" },
    ],
  },
  {
    key: "step4",
    slides: [
      { src: m4a, captionKey: "slide0" },
      { src: m4b, captionKey: "slide1" },
    ],
  },
  { key: "key", slides: [] },
  { key: "model", slides: [] },
];

const DESKTOP_STEPS: GuidedStep[] = [
  { key: "step1", openUrl: OPENROUTER_URL, slides: [{ src: d1a, captionKey: "slide0" }] },
  {
    key: "step2",
    slides: [
      { src: d2a, captionKey: "slide0" },
      { src: d2b, captionKey: "slide1" },
    ],
  },
  {
    key: "step3",
    slides: [
      { src: d3a, captionKey: "slide0" },
      { src: d3b, captionKey: "slide1" },
    ],
  },
  {
    key: "step4",
    slides: [
      { src: d4a, captionKey: "slide0" },
      { src: d4b, captionKey: "slide1" },
    ],
  },
  { key: "key", slides: [] },
  { key: "model", slides: [] },
];

interface RemoteModel {
  id: string;
  displayName?: string | null;
  inputPrice?: number | null;
  outputPrice?: number | null;
}

interface RecommendedFamily {
  label: string;
  tokens: string[];
  prefer: string[];
  recommended?: boolean;
}

const FAMILIES: RecommendedFamily[] = [
  { label: "GLM 5", tokens: ["glm"], prefer: ["glm-5", "glm5"], recommended: true },
  { label: "DeepSeek", tokens: ["deepseek"], prefer: ["v4", "v3.2", "v3"] },
  { label: "Kimi K2", tokens: ["kimi"], prefer: ["k2.5", "k2"] },
  { label: "Qwen", tokens: ["qwen"], prefer: ["max", "235", "plus"] },
  { label: "MiniMax", tokens: ["minimax"], prefer: ["m2", "m1"] },
];

function pickBest(models: RemoteModel[], family: RecommendedFamily): RemoteModel | null {
  const candidates = models.filter((m) => {
    const id = m.id.toLowerCase();
    if (/claude|anthropic/.test(id)) return false;
    return family.tokens.some((tok) => id.includes(tok));
  });
  if (candidates.length === 0) return null;

  const score = (m: RemoteModel): number => {
    const id = m.id.toLowerCase();
    let s = 0;
    const preferIdx = family.prefer.findIndex((p) => id.includes(p));
    if (preferIdx >= 0) s += (family.prefer.length - preferIdx) * 100;
    if (id.endsWith(":free")) s -= 50;
    if (/distill|lite|mini|small|air/.test(id)) s -= 20;
    s -= id.length * 0.1;
    return s;
  };

  return [...candidates].sort((a, b) => score(b) - score(a))[0];
}

function formatPrice(perToken?: number | null): string | null {
  if (perToken == null || !Number.isFinite(perToken)) return null;
  if (perToken === 0) return "$0";
  const perMillion = perToken * 1_000_000;
  return perMillion < 0.1 ? `$${perMillion.toFixed(3)}` : `$${perMillion.toFixed(2)}`;
}

function ModelChoiceStep({
  credentialId,
  credentialLabel,
}: {
  credentialId: string;
  credentialLabel: string;
}) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [cards, setCards] = useState<Array<{ family: RecommendedFamily; model: RemoteModel }>>([]);
  const [creating, setCreating] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const models = await invoke<RemoteModel[]>("get_remote_models", { credentialId });
      const built = FAMILIES.map((family) => {
        const model = pickBest(models, family);
        return model ? { family, model } : null;
      }).filter((x): x is { family: RecommendedFamily; model: RemoteModel } => x !== null);
      setCards(built);
    } catch {
      setError(t("onboarding.openrouter.model.loadFailed"));
    } finally {
      setLoading(false);
    }
  }, [credentialId, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleSelect = async (model: RemoteModel) => {
    if (creating) return;
    setCreating(model.id);
    try {
      await addOrUpdateModel({
        name: model.id,
        providerId: "openrouter",
        providerCredentialId: credentialId,
        providerLabel: credentialLabel,
        displayName: model.displayName || model.id.split("/").pop() || model.id,
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
      setError(t("onboarding.openrouter.model.createFailed"));
      setCreating(null);
    }
  };

  if (loading) {
    return (
      <div className="mt-6 flex flex-col items-center gap-3 py-10 text-white/60">
        <Loader2 size={22} className="animate-spin" />
        <span className="text-[13px]">{t("onboarding.openrouter.model.loading")}</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="mt-6 space-y-3">
        <p className="flex items-start gap-1.5 rounded-lg border border-red-400/30 bg-red-950/60 px-3 py-2 text-[13px] leading-relaxed text-red-200 shadow-lg backdrop-blur-md">{error}</p>
        <button
          type="button"
          onClick={() => void load()}
          className={cn(
            "w-full border border-white/15 bg-black/55 shadow-lg backdrop-blur-md py-3 text-[14px] font-medium text-white transition hover:bg-black/65",
            radius.md,
          )}
        >
          {t("common.buttons.retry")}
        </button>
      </div>
    );
  }

  return (
    <div className="mt-4">
      <p className="mb-4 text-[14px] leading-relaxed text-white/65">
        {t("onboarding.openrouter.model.body")}
      </p>
      <div className="space-y-2.5">
        {cards.map(({ family, model }) => {
          const inPrice = formatPrice(model.inputPrice);
          const outPrice = formatPrice(model.outputPrice);
          const isCreating = creating === model.id;
          return (
            <button
              key={model.id}
              type="button"
              disabled={!!creating}
              onClick={() => void handleSelect(model)}
              className={cn(
                "flex w-full items-center gap-3 border p-3.5 text-left transition active:scale-[0.99] disabled:opacity-60",
                radius.lg,
                family.recommended
                  ? "border-emerald-500/40 bg-emerald-500/10 hover:bg-emerald-500/15"
                  : "border-white/15 bg-black/40 hover:border-white/25 hover:bg-black/55",
              )}
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-[15px] font-semibold text-white">
                    {family.label}
                  </span>
                  {family.recommended && (
                    <span className="shrink-0 rounded border border-emerald-500/20 bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-emerald-400">
                      {t("onboarding.openrouter.model.recommended")}
                    </span>
                  )}
                </div>
                <p className="mt-0.5 truncate text-[12px] text-white/45">{model.id}</p>
                {(inPrice || outPrice) && (
                  <p className="mt-1 text-[12px] text-white/55">
                    {inPrice && (
                      <span>
                        {inPrice} {t("onboarding.openrouter.model.perInput")}
                      </span>
                    )}
                    {inPrice && outPrice && <span className="text-white/25"> · </span>}
                    {outPrice && (
                      <span>
                        {outPrice} {t("onboarding.openrouter.model.perOutput")}
                      </span>
                    )}
                  </p>
                )}
              </div>
              <div className="shrink-0 text-white/30">
                {isCreating ? (
                  <Loader2 size={18} className="animate-spin text-emerald-300" />
                ) : (
                  <ArrowRight size={18} />
                )}
              </div>
            </button>
          );
        })}
      </div>
      <p className="mt-4 text-center text-[12px] text-white/40">
        {t("onboarding.openrouter.model.footnote")}
      </p>
    </div>
  );
}

function ApiKeyStep({ onSaved }: { onSaved: (credentialId: string, label: string) => void }) {
  const { t } = useI18n();
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
        { providerId: "openrouter", credentialId, apiKey: trimmed, baseUrl: null },
      );
      if (!verification.valid) {
        setError(t("onboarding.openrouter.key.invalid"));
        setBusy(false);
        return;
      }
      const credential = await addOrUpdateProviderCredential({
        id: credentialId,
        providerId: "openrouter",
        label: "OpenRouter",
        apiKey: trimmed,
      });
      onSaved(credential.id, credential.label);
    } catch {
      setError(t("onboarding.openrouter.key.failed"));
      setBusy(false);
    }
  };

  return (
    <div className="mt-4 flex flex-col">
      <p className="mb-5 text-[14px] leading-relaxed text-white/65">
        {t("onboarding.openrouter.key.body")}
      </p>
      <input
        type="text"
        value={apiKey}
        onChange={(e) => {
          setApiKey(e.target.value);
          if (error) setError(null);
        }}
        placeholder={t("onboarding.openrouter.key.placeholder")}
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
            {t("onboarding.openrouter.key.connecting")}
          </>
        ) : (
          <>
            <Check size={16} />
            {t("onboarding.openrouter.key.connect")}
          </>
        )}
      </button>
    </div>
  );
}

export function OpenRouterSetupStep({ onExitBack }: { onExitBack: () => void }) {
  const { t } = useI18n();
  const isDesktop = getPlatform().type === "desktop";
  const variant = isDesktop ? "desktop" : "mobile";
  const STEPS = isDesktop ? DESKTOP_STEPS : MOBILE_STEPS;

  const [stepIndex, setStepIndex] = useState(0);
  const [direction, setDirection] = useState(1);
  const [credential, setCredential] = useState<{ id: string; label: string } | null>(null);
  const step = STEPS[Math.min(stepIndex, STEPS.length - 1)];
  const isKeyStep = step.key === "key";
  const isModelStep = step.key === "model";
  const isLastScreenshot = stepIndex === STEPS.length - 3;
  const centered = isDesktop || isKeyStep || isModelStep;

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
    ? "onboarding.openrouter.key.title"
    : isModelStep
      ? "onboarding.openrouter.model.title"
      : `onboarding.openrouter.${variant}.${step.key}.title`;

  const lead = (
    <div className="flex items-center gap-2.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-white/10 [&>img]:h-5 [&>img]:w-5 [&>img]:object-contain">
        {getProviderIcon("openrouter")}
      </div>
      <span className="text-[11px] font-semibold uppercase tracking-[0.2em] text-emerald-300/90 [text-shadow:0_1px_10px_rgba(0,0,0,0.85)]">
        {t("onboarding.steps.stepNofM", { current: stepIndex + 1, total: STEPS.length })}
      </span>
    </div>
  );

  const titleEl = (
    <h1 className="mt-3 text-[22px] font-semibold leading-tight text-white [text-shadow:0_1px_12px_rgba(0,0,0,0.9)] lg:text-[28px]">
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
      {t(`onboarding.openrouter.${variant}.${step.key}.body` as TranslationKey)}
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
          {t("onboarding.openrouter.openSite")}
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
        {isLastScreenshot ? t("onboarding.openrouter.key.title") : t("common.buttons.next")}
        <ArrowRight size={16} />
      </button>
    </div>
  );

  const backButton = (
    <button
      type="button"
      onClick={goPrev}
      className="mt-4 flex items-center gap-1.5 self-center text-[13px] font-medium text-white/70 [text-shadow:0_1px_8px_rgba(0,0,0,0.7)] transition hover:text-white/70"
    >
      <ArrowLeft size={14} />
      {t("common.buttons.back")}
    </button>
  );

  let containerClass: string;
  let inner: ReactNode;

  if (isKeyStep) {
    containerClass = "mx-auto flex w-full max-w-md flex-1 flex-col px-4";
    inner = (
      <>
        {lead}
        {titleEl}
        {progress}
        <ApiKeyStep
          onSaved={(id, label) => {
            setCredential({ id, label });
            goNext();
          }}
        />
        {backButton}
      </>
    );
  } else if (isModelStep) {
    containerClass = "mx-auto flex w-full max-w-md flex-1 flex-col px-4";
    inner = (
      <>
        {lead}
        {titleEl}
        {progress}
        {credential && (
          <ModelChoiceStep credentialId={credential.id} credentialLabel={credential.label} />
        )}
      </>
    );
  } else if (isDesktop) {
    containerClass = "mx-auto flex w-full max-w-6xl flex-1 flex-col px-6";
    inner = (
      <div className="grid items-center gap-12 lg:grid-cols-[minmax(0,340px)_minmax(0,1fr)]">
        <div>
          {lead}
          {titleEl}
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
          captionBase={`onboarding.openrouter.${variant}.${step.key}`}
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
        {titleEl}
        {progress}
        {body}
        <div className="mt-6">
          <GuidedCarousel
            captionBase={`onboarding.openrouter.${variant}.${step.key}`}
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
