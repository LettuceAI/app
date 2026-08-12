import { useState } from "react";
import {
  AppWindow,
  Building2,
  KeyRound,
  Cpu,
  Wallet,
  ArrowLeft,
  ArrowRight,
  Lightbulb,
  BookOpen,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn, typography } from "../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { openDocs } from "../../../../core/utils/docs";
import logoSvg from "../../../../assets/logo.svg";

interface LearnStepProps {
  onDone: () => void;
  onExitBack: () => void;
}

const CARDS: Array<{ icon?: LucideIcon; key: string }> = [
  { key: "welcome" },
  { icon: AppWindow, key: "app" },
  { icon: Building2, key: "provider" },
  { icon: KeyRound, key: "apiKey" },
  { icon: Cpu, key: "model" },
  { icon: Wallet, key: "cost" },
];

export function LearnStep({ onDone, onExitBack }: LearnStepProps) {
  const { t } = useI18n();
  const [index, setIndex] = useState(0);

  const card = CARDS[index];
  const Icon = card.icon;
  const isFirst = index === 0;
  const isLast = index === CARDS.length - 1;

  const goPrev = () => {
    if (isFirst) onExitBack();
    else setIndex((i) => i - 1);
  };
  const goNext = () => {
    if (isLast) onDone();
    else setIndex((i) => i + 1);
  };

  return (
    <div className="mx-auto flex min-h-full w-full max-w-xl flex-1 flex-col pb-8">
      <div className="mb-6 flex items-center justify-center gap-1.5">
        {CARDS.map((c, i) => (
          <span
            key={c.key}
            className={cn(
              "h-1.5 rounded-full transition-all duration-300",
              i === index ? "w-6 bg-emerald-400" : "w-1.5 bg-white/25",
            )}
          />
        ))}
      </div>

      <div className="flex flex-1 flex-col items-center justify-center text-center">
        {card.key === "welcome" ? (
          <img src={logoSvg} alt="" className="mb-5 h-24 w-24 drop-shadow-[0_2px_16px_rgba(0,0,0,0.6)]" />
        ) : (
          <div className="mb-5 flex h-16 w-16 items-center justify-center rounded-2xl border border-emerald-400/30 bg-black/40 text-emerald-300 backdrop-blur-md">
            {Icon ? <Icon size={30} strokeWidth={1.8} /> : null}
          </div>
        )}
        <p className="mb-2 text-[11px] font-semibold uppercase tracking-[0.25em] text-emerald-300/90 [text-shadow:0_1px_10px_rgba(0,0,0,0.8)]">
          {t(`onboarding.learn.${card.key}.tag` as TranslationKey)}
        </p>
        <h1
          className={cn(
            typography.h1.size,
            typography.h1.weight,
            "text-white [text-shadow:0_1px_16px_rgba(0,0,0,0.85)]",
          )}
        >
          {t(`onboarding.learn.${card.key}.title` as TranslationKey)}
        </h1>
        <p className="mt-3 max-w-md text-[15px] leading-relaxed text-white/85 [text-shadow:0_1px_12px_rgba(0,0,0,0.75)]">
          {t(`onboarding.learn.${card.key}.body` as TranslationKey)}
        </p>

        {card.key !== "welcome" && (
          <div className="mt-6 w-full max-w-md rounded-2xl border border-white/12 bg-black/60 p-4 text-left shadow-lg backdrop-blur-md">
            <div className="mb-1.5 flex items-center gap-1.5 text-emerald-300/90">
              <Lightbulb size={13} strokeWidth={2} />
              <span className="text-[10px] font-semibold uppercase tracking-[0.18em]">
                {t("onboarding.learn.plainLabel")}
              </span>
            </div>
            <p className="text-[13px] leading-relaxed text-white/80">
              {t(`onboarding.learn.${card.key}.plain` as TranslationKey)}
            </p>
          </div>
        )}

        <button
          onClick={() => void openDocs("aiBasics")}
          className={cn(
            "mt-6 inline-flex items-center gap-2 rounded-full px-4 py-2",
            "border border-emerald-400/30 bg-emerald-500/10 shadow-lg backdrop-blur-md",
            "text-[12px] font-semibold text-emerald-200/90",
            "transition hover:border-emerald-400/55 hover:bg-emerald-500/20 hover:text-emerald-100 active:scale-[0.98]",
          )}
        >
          <BookOpen size={14} strokeWidth={2} />
          {t("onboarding.learn.fullGuide")}
        </button>
      </div>

      <div className="mt-8 flex items-center gap-3">
        <button
          onClick={goPrev}
          className="flex items-center justify-center gap-2 rounded-xl border border-white/15 bg-black/50 px-5 py-3 text-[14px] font-medium text-white/85 shadow-lg backdrop-blur-md transition hover:border-white/25 hover:bg-black/60 active:scale-[0.98]"
        >
          <ArrowLeft size={16} />
          {t("common.buttons.back")}
        </button>
        <button
          onClick={goNext}
          className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-emerald-400/50 bg-emerald-500/35 px-5 py-3 text-[14px] font-semibold text-emerald-50 shadow-lg backdrop-blur-md transition hover:border-emerald-400/70 hover:bg-emerald-500/45 active:scale-[0.98]"
        >
          {isLast ? t("onboarding.learn.done") : t("common.buttons.next")}
          <ArrowRight size={16} />
        </button>
      </div>
    </div>
  );
}
