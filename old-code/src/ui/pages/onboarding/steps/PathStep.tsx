import { ArrowRight, Check, Minus } from "lucide-react";
import { cn, radius, typography } from "../../../design-tokens";
import { getProviderIcon } from "../../../../core/utils/providerIcons";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";

interface PathStepProps {
  onChoose: (path: "free" | "paid") => void;
}

function PathCard({
  variant,
  onClick,
}: {
  variant: "free" | "paid";
  onClick: () => void;
}) {
  const { t } = useI18n();
  const providerIcon = getProviderIcon(variant === "free" ? "gemini" : "openrouter");
  const pros = [0, 1, 2];
  const cons = [0, 1];

  return (
    <div
      className={cn(
        "flex flex-col border border-white/15 bg-black/65 p-5",
        "sm:row-span-4 sm:grid sm:grid-rows-subgrid",
        radius.lg,
      )}
    >
      <div className="flex items-center gap-3 pb-4">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-white/10 bg-white/10 [&>img]:h-6 [&>img]:w-6 [&>img]:object-contain">
          {providerIcon}
        </div>
        <div className="min-w-0 flex-1">
          <h3 className="text-[18px] font-semibold text-white">
            {t(`onboarding.path.${variant}.title` as TranslationKey)}
          </h3>
          <p className="mt-0.5 text-[13px] text-white/50">
            {t(`onboarding.path.${variant}.subtitle` as TranslationKey)}
          </p>
        </div>
      </div>

      <div className="space-y-2.5 border-t border-white/8 pt-4">
        <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-white/40">
          {t("onboarding.path.prosLabel")}
        </p>
        {pros.map((i) => (
          <div key={`pro-${i}`} className="flex items-start gap-2.5 text-[13px] text-white/80">
            <Check size={15} className="mt-px shrink-0 text-emerald-400" />
            <span className="leading-snug">
              {t(`onboarding.path.${variant}.pro${i}` as TranslationKey)}
            </span>
          </div>
        ))}
      </div>

      <div className="mt-4 space-y-2.5 border-t border-white/8 pt-4 sm:mt-0">
        <p className="text-[10px] font-medium uppercase tracking-[0.18em] text-white/40">
          {t("onboarding.path.consLabel")}
        </p>
        {cons.map((i) => (
          <div key={`con-${i}`} className="flex items-start gap-2.5 text-[13px] text-white/45">
            <Minus size={15} className="mt-px shrink-0 text-white/30" />
            <span className="leading-snug">
              {t(`onboarding.path.${variant}.con${i}` as TranslationKey)}
            </span>
          </div>
        ))}
      </div>

      <button
        onClick={onClick}
        className={cn(
          "mt-5 flex items-center justify-center gap-2 self-end py-2.5 text-[14px] font-semibold transition active:scale-[0.99]",
          "w-full sm:mt-4",
          radius.md,
          "border",
          variant === "free"
            ? "border-emerald-500/50 bg-emerald-500/30 text-emerald-200 hover:bg-emerald-500/40"
            : "border-white/20 bg-white/15 text-white hover:border-white/30 hover:bg-white/20",
        )}
      >
        {t(`onboarding.path.${variant}.cta` as TranslationKey)}
        <ArrowRight size={16} />
      </button>
    </div>
  );
}

export function PathStep({ onChoose }: PathStepProps) {
  const { t } = useI18n();

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col pb-10">
      <div className="mb-7 space-y-2 text-center">
        <h1 className={cn(typography.h1.size, typography.h1.weight, "text-white")}>
          {t("onboarding.path.title")}
        </h1>
        <p className="text-[15px] leading-relaxed text-white/60">
          {t("onboarding.path.body")}
        </p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 sm:grid-rows-[auto_auto_auto_auto] sm:gap-y-0">
        <PathCard variant="free" onClick={() => onChoose("free")} />
        <PathCard variant="paid" onClick={() => onChoose("paid")} />
      </div>

      <p className="mt-5 text-center text-[12px] text-white/40">
        {t("onboarding.path.footnote")}
      </p>
    </div>
  );
}
