import { Sparkles, GraduationCap, ArrowRight, KeyRound } from "lucide-react";
import { cn, typography } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";

interface IntroStepProps {
  onFirstTime: () => void;
  onExperienced: () => void;
}

export function IntroStep({ onFirstTime, onExperienced }: IntroStepProps) {
  const { t } = useI18n();

  return (
    <div className="mx-auto flex min-h-full w-full max-w-xl flex-1 flex-col justify-center pb-10">
      <div className="mb-7 flex flex-col items-center text-center">
        <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-white/15 bg-black/40 text-emerald-300 backdrop-blur-md">
          <KeyRound size={26} strokeWidth={2} />
        </div>
        <h1
          className={cn(
            typography.h1.size,
            typography.h1.weight,
            "mt-4 text-white [text-shadow:0_1px_16px_rgba(0,0,0,0.85)]",
          )}
        >
          {t("onboarding.intro.title")}
        </h1>
        <p className="mt-3 max-w-md text-[15px] leading-relaxed text-white/85 [text-shadow:0_1px_12px_rgba(0,0,0,0.75)]">
          {t("onboarding.intro.body")}
        </p>
      </div>

      <div className="space-y-3">
        <button
          onClick={onFirstTime}
          className={cn(
            "group w-full rounded-2xl border border-emerald-400/45 bg-emerald-950/55 p-5 text-left shadow-lg backdrop-blur-md",
            "transition hover:border-emerald-400/65 hover:bg-emerald-900/60 active:scale-[0.99]",
          )}
        >
          <div className="flex items-center gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-emerald-400/30 bg-emerald-500/20 text-emerald-300">
              <GraduationCap size={22} />
            </div>
            <div className="min-w-0 flex-1">
              <h3 className="text-[16px] font-semibold text-white">
                {t("onboarding.intro.firstTimeTitle")}
              </h3>
              <p className="mt-0.5 text-[13px] leading-relaxed text-white/65">
                {t("onboarding.intro.firstTimeDesc")}
              </p>
            </div>
            <ArrowRight size={18} className="shrink-0 text-emerald-300/80" />
          </div>
        </button>

        <button
          onClick={onExperienced}
          className={cn(
            "group w-full rounded-2xl border border-white/15 bg-black/50 p-5 text-left shadow-lg backdrop-blur-md",
            "transition hover:border-white/25 hover:bg-black/60 active:scale-[0.99]",
          )}
        >
          <div className="flex items-center gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-white/10 bg-white/10 text-white/75">
              <Sparkles size={22} />
            </div>
            <div className="min-w-0 flex-1">
              <h3 className="text-[16px] font-semibold text-white">
                {t("onboarding.intro.experiencedTitle")}
              </h3>
              <p className="mt-0.5 text-[13px] leading-relaxed text-white/60">
                {t("onboarding.intro.experiencedDesc")}
              </p>
            </div>
            <ArrowRight size={18} className="shrink-0 text-white/45" />
          </div>
        </button>
      </div>
    </div>
  );
}
