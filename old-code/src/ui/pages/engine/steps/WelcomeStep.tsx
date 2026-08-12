import { Leaf, ArrowRight } from "lucide-react";
import { useI18n } from "../../../../core/i18n/context";

type Props = {
  onNext: () => void;
};

export function WelcomeStep({ onNext }: Props) {
  const { t } = useI18n();
  return (
    <div className="flex flex-col items-center px-4 py-8">
      <div className="flex h-20 w-20 items-center justify-center rounded-3xl border border-emerald-400/30 bg-emerald-500/15">
        <Leaf className="h-10 w-10 text-emerald-300" />
      </div>
      <h1 className="mt-6 text-2xl font-bold text-white">{t("engine.welcome.title")}</h1>
      <p className="mt-2 text-center text-sm text-white/60">
        {t("engine.welcome.subtitle")}
      </p>
      <div className="mt-8 w-full max-w-sm space-y-3">
        <div className="rounded-xl border border-white/10 bg-white/5 p-4">
          <p className="text-sm text-white/80">
            {t("engine.welcome.feature1")}
          </p>
        </div>
        <div className="rounded-xl border border-white/10 bg-white/5 p-4">
          <p className="text-sm text-white/80">
            {t("engine.welcome.feature2")}
          </p>
        </div>
      </div>
      <button
        onClick={onNext}
        className="mt-8 flex items-center gap-2 rounded-full border border-emerald-400/40 bg-emerald-500/20 px-8 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 active:scale-[0.98]"
      >
        {t("engine.welcome.getStarted")}
        <ArrowRight className="h-4 w-4" />
      </button>
    </div>
  );
}
