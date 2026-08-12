import { ArrowLeft, Sparkles, Zap, DollarSign, Brain } from "lucide-react";
import { useI18n } from "../../../core/i18n/context";

type FactorCard = {
  icon: "sparkles" | "zap" | "dollar" | "brain";
  title: string;
  description: string;
  questions: string[];
};

const iconMap = {
  sparkles: Sparkles,
  zap: Zap,
  dollar: DollarSign,
  brain: Brain,
};

interface ModelRecommendationsProps {
  onBack: () => void;
}

export function ModelRecommendations({ onBack }: ModelRecommendationsProps) {
  const { t } = useI18n();

  const factors: FactorCard[] = [
    {
      icon: "sparkles",
      title: t("onboarding.modelGuide.factors.quality.title"),
      description: t("onboarding.modelGuide.factors.quality.description"),
      questions: [
        t("onboarding.modelGuide.factors.quality.q1"),
        t("onboarding.modelGuide.factors.quality.q2"),
        t("onboarding.modelGuide.factors.quality.q3"),
      ],
    },
    {
      icon: "zap",
      title: t("onboarding.modelGuide.factors.speed.title"),
      description: t("onboarding.modelGuide.factors.speed.description"),
      questions: [
        t("onboarding.modelGuide.factors.speed.q1"),
        t("onboarding.modelGuide.factors.speed.q2"),
        t("onboarding.modelGuide.factors.speed.q3"),
      ],
    },
    {
      icon: "dollar",
      title: t("onboarding.modelGuide.factors.budget.title"),
      description: t("onboarding.modelGuide.factors.budget.description"),
      questions: [
        t("onboarding.modelGuide.factors.budget.q1"),
        t("onboarding.modelGuide.factors.budget.q2"),
        t("onboarding.modelGuide.factors.budget.q3"),
        t("onboarding.modelGuide.factors.budget.q4"),
      ],
    },
    {
      icon: "brain",
      title: t("onboarding.modelGuide.factors.safety.title"),
      description: t("onboarding.modelGuide.factors.safety.description"),
      questions: [
        t("onboarding.modelGuide.factors.safety.q1"),
        t("onboarding.modelGuide.factors.safety.q2"),
        t("onboarding.modelGuide.factors.safety.q3"),
      ],
    },
  ];

  return (
    <div className="flex min-h-screen flex-col bg-black text-white px-4 pb-8 pt-[calc(env(safe-area-inset-top)+12px)]">
      {/* Header */}
      <div className="mb-6 flex items-center justify-between">
        <button
          onClick={onBack}
          className="flexitems-center justify-center rounded-full border border-white/10 bg-white/10 text-white hover:border-white/25 hover:bg-white/15 active:scale-95 transition"
          aria-label={t("common.buttons.goBack")}
        >
          <ArrowLeft size={10} />
        </button>
        <div className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[12px] uppercase tracking-[0.15em] text-white/60">
          {t("onboarding.modelGuide.badge")}
        </div>
        <div className="w-10" />
      </div>

      {/* Title & intro */}
      <h1 className="text-[25px] font-semibold leading-tight text-white">{t("onboarding.modelGuide.title")}</h1>
      <p className="mt-2 text-[15px] text-white/65">
        {(() => {
          const parts = t("onboarding.modelGuide.intro").split(/<0>|<\/0>/);
          return (
            <>
              {parts[0]}
              <span className="font-medium text-white/80">{parts[1]}</span>
              {parts[2]}
            </>
          );
        })()}
      </p>

      {/* Factors */}
      <div className="mt-6 space-y-4">
        {factors.map((factor) => {
          const Icon = iconMap[factor.icon];
          return (
            <div
              key={factor.title}
              className="rounded-2xl border border-white/10 bg-white/5 p-4 hover:border-white/20 hover:bg-white/8 transition"
            >
              <div className="flex items-start gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-linear-to-br from-green-500/20 to-darkgreen-500/20 border border-white/10 shrink-0">
                  <Icon size={20} className="text-green-300" />
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="text-[17px] font-semibold text-white mb-1">{factor.title}</h3>
                  <p className="text-[15px] text-white/70 mb-3 leading-relaxed">{factor.description}</p>

                  <div>
                    <p className="text-[13px] font-medium text-white/50 mb-1.5">{t("onboarding.modelGuide.askYourself")}</p>
                    <ul className="space-y-1.5 text-[13px] text-white/75">
                      {factor.questions.map((q) => (
                        <li key={q} className="flex gap-1.5">
                          <span className="mt-[3px] h-1 w-1 rounded-full bg-white/40 shrink-0" />
                          <span>{q}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Where to look for models */}
      <div className="mt-8 rounded-2xl border border-white/10 bg-linear-to-br from-white/5 to-transparent p-4 space-y-3">
        <h2 className="text-[15px] font-semibold text-white">{t("onboarding.modelGuide.where.title")}</h2>
        <p className="text-[13px] text-white/70">
          {(() => {
            const parts = t("onboarding.modelGuide.where.intro").split(/<0>|<\/0>/);
            return (
              <>
                {parts[0]}
                <span className="font-medium text-white/85">{parts[1]}</span>
                {parts[2]}
              </>
            );
          })()}
        </p>

        <div className="mt-2 grid grid-cols-1 gap-2 text-[13px] text-white/75">
          <div className="rounded-xl border border-white/10 bg-black/40 px-3 py-2">
            <p className="font-medium text-white/85 mb-0.5">{t("onboarding.modelGuide.where.directTitle")}</p>
            <p className="text-white/60">
              {t("onboarding.modelGuide.where.directDesc")}
            </p>
          </div>
          <div className="rounded-xl border border-white/10 bg-black/40 px-3 py-2">
            <p className="font-medium text-white/85 mb-0.5">{t("onboarding.modelGuide.where.routersTitle")}</p>
            <p className="text-white/60">
              {t("onboarding.modelGuide.where.routersDesc")}
            </p>
          </div>
          <div className="rounded-xl border border-white/10 bg-black/40 px-3 py-2">
            <p className="font-medium text-white/85 mb-0.5">{t("onboarding.modelGuide.where.communityTitle")}</p>
            <p className="text-white/60">
              {t("onboarding.modelGuide.where.communityDesc")}
            </p>
          </div>
        </div>
      </div>

      {/* Rules of thumb */}
      <div className="mt-6 rounded-2xl border border-white/10 bg-white/5 p-4 space-y-2">
        <h2 className="text-[15px] font-semibold text-white">{t("onboarding.modelGuide.rules.title")}</h2>
        <ul className="space-y-1.5 text-[13px] text-white/70">
          <li>• For casual chatting → pick a fast, cheap chat model from your provider/router.</li>
          <li>
            • For experiments or high volume → start with the cheapest model that feels good enough,
            then upgrade if needed.
          </li>
          <li>
            • If something feels off (too slow / too dumb / too expensive) → you can always switch
            models later in LettuceAI.
          </li>
        </ul>
      </div>

      <div className="mt-4 text-[12px] text-white/45">
        {t("onboarding.modelGuide.disclaimer")}
      </div>
    </div>
  );
}
