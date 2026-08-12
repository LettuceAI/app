import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { motion } from "framer-motion";
import { BrainCircuit, Download, Check, ShieldCheck, Loader2 } from "lucide-react";
import { cn, radius } from "../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { setOnboardingCompleted } from "../../../../core/storage/appState";

const HOME_WITH_TOUR = "/chat?firstTime=true";
const EMBEDDING_DOWNLOAD = `/settings/embedding-download?auto=1&capacity=2048&returnTo=${encodeURIComponent(
  HOME_WITH_TOUR,
)}`;

export function MemorySetupStep() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [busy, setBusy] = useState(false);

  const handleDownload = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await setOnboardingCompleted(true);
    } catch {
      // proceed regardless; the download page will still run.
    }
    navigate(EMBEDDING_DOWNLOAD);
  };

  const handleSkip = async () => {
    if (busy) return;
    try {
      await setOnboardingCompleted(true);
    } catch {
      // proceed regardless.
    }
    navigate(HOME_WITH_TOUR);
  };

  const points: Array<{ icon: typeof Check; key: string }> = [
    { icon: Check, key: "point0" },
    { icon: ShieldCheck, key: "point1" },
    { icon: Download, key: "point2" },
  ];

  return (
    <div className="mx-auto flex min-h-full w-full max-w-md flex-1 flex-col justify-center pb-10">
      <motion.div
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.2, ease: "easeOut" }}
        className="flex flex-col"
      >
        <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl border border-emerald-500/30 bg-emerald-500/10 text-emerald-300">
          <BrainCircuit size={28} strokeWidth={1.8} />
        </div>

        <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-emerald-300/90 [text-shadow:0_1px_10px_rgba(0,0,0,0.85)]">
          {t("onboarding.memorySetup.kicker")}
        </p>
        <h1 className="mt-2 text-[24px] font-semibold leading-tight text-white [text-shadow:0_1px_12px_rgba(0,0,0,0.9)]">
          {t("onboarding.memorySetup.title")}
        </h1>
        <p className="mt-4 text-[15px] leading-relaxed text-white/75 [text-shadow:0_1px_10px_rgba(0,0,0,0.7)]">
          {t("onboarding.memorySetup.body")}
        </p>

        <div className="mt-5 space-y-2.5">
          {points.map(({ icon: Icon, key }) => (
            <div key={key} className="flex items-start gap-2.5 text-[13px] text-white/80">
              <Icon size={15} className="mt-px shrink-0 text-emerald-400" />
              <span className="leading-snug [text-shadow:0_1px_8px_rgba(0,0,0,0.7)]">
                {t(`onboarding.memorySetup.${key}` as TranslationKey)}
              </span>
            </div>
          ))}
        </div>

        <button
          type="button"
          onClick={() => void handleDownload()}
          disabled={busy}
          className={cn(
            "mt-7 flex items-center justify-center gap-2 border border-emerald-500/50 bg-emerald-500/25 py-3 text-[14px] font-semibold text-emerald-100 transition hover:bg-emerald-500/35 active:scale-[0.99]",
            radius.md,
            "disabled:cursor-not-allowed disabled:opacity-60",
          )}
        >
          {busy ? <Loader2 size={16} className="animate-spin" /> : <Download size={16} />}
          {t("onboarding.memorySetup.download")}
        </button>

        <button
          type="button"
          onClick={() => void handleSkip()}
          disabled={busy}
          className="mt-3 self-center text-[13px] font-medium text-white/45 transition hover:text-white/70 disabled:opacity-60"
        >
          {t("onboarding.memorySetup.skip")}
        </button>
      </motion.div>
    </div>
  );
}
