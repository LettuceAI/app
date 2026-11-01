import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { ArrowUpRight } from "lucide-react";
import { Navigate, useLocation, useNavigate } from "react-router-dom";

import logo from "../../../assets/logo.svg";
import { getAppUpdateDismissKey, type AppUpdateInfo } from "../../../core/app-updates/checkForAppUpdate";
import { useI18n } from "../../../core/i18n/context";
import { setTooltipSeen } from "../../../core/storage/appState";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { getPlatform } from "../../../core/utils/platform";
import Grainient from "./Grainient";

const ease = [0.22, 1, 0.36, 1] as const;
const colors = ["#020403", "#06231a", "#1c6b4c", "#5f9a3a", "#030806"];

type ReleaseLocationState = { update?: AppUpdateInfo; returnTo?: string };

export function NextReleasePage() {
  const { t } = useI18n();
  const location = useLocation();
  const navigate = useNavigate();
  const reducedMotion = useReducedMotion();
  const [hello, setHello] = useState(!reducedMotion);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const state = location.state as ReleaseLocationState | null;
  const update = state?.update;
  const allowed = update?.isNextRelease === true;
  const returnTo = state?.returnTo?.startsWith("/") &&
    !state.returnTo.startsWith("//") && !state.returnTo.startsWith("/welcome-next")
    ? state.returnTo : "/";

  useEffect(() => {
    if (!allowed) return;
    headingRef.current?.focus({ preventScroll: true });
    const timer = window.setTimeout(() => setHello(false), reducedMotion ? 0 : 2200);
    return () => window.clearTimeout(timer);
  }, [allowed, reducedMotion]);

  useEffect(() => {
    if (!allowed || hello || !update) return;
    void setTooltipSeen(getAppUpdateDismissKey(update, getPlatform().os), true)
      .catch((error) => console.error("Failed to remember Next announcement:", error));
  }, [allowed, hello, update]);

  useEffect(() => {
    if (!allowed) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      navigate(returnTo, { replace: true });
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [allowed, navigate, returnTo]);

  if (!allowed) return <Navigate to="/" replace />;

  const reveal = { duration: reducedMotion ? 0 : 0.8, ease };

  return (
    <section
      aria-labelledby="next-release-title"
      className="relative isolate flex h-[calc(100dvh-var(--titlebar-h,0px))] min-h-0 flex-col overflow-y-auto bg-[#030504] text-white [scrollbar-width:none]"
    >
      <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden bg-[radial-gradient(ellipse_at_60%_70%,#1c6b4c_0%,#06231a_40%,#030504_80%)]">
        <Grainient
          colors={colors}
          angle={55}
          flow={0.8}
          scale={1}
          speed={0.45}
          contrast={1.1}
          saturation={0.9}
          grainAmount={0.06}
          interactive={false}
          clickSwirl={false}
          dpr={1.5}
        />
        <div className="absolute inset-0 bg-black/15" />
      </div>

      <header className="relative flex shrink-0 items-center justify-between gap-4 px-6 pb-5 pt-[calc(env(safe-area-inset-top)+1.5rem)] sm:px-10 sm:pt-8">
        <img src={logo} alt="LettuceAI" className="h-8 w-8 sm:h-9 sm:w-9" />
        <span className="text-right text-[10px] font-medium uppercase tracking-[0.2em] text-white/60 sm:text-xs">
          {t("updates.nextRelease.available")}
        </span>
      </header>

      <div className="relative mx-auto flex w-full max-w-4xl flex-1 flex-col items-center justify-center px-6 py-8 text-center sm:px-10">
        <motion.h1
          ref={headingRef}
          id="next-release-title"
          tabIndex={-1}
          layout={!reducedMotion}
          initial={reducedMotion ? false : { opacity: 0, filter: "blur(12px)", scale: 0.96 }}
          animate={{ opacity: 1, filter: "blur(0px)", scale: 1 }}
          transition={{ duration: reducedMotion ? 0 : 1.2, ease, layout: { duration: 0.9, ease } }}
          className="text-[clamp(2.25rem,7.5vw,5rem)] font-bold leading-[1.05] tracking-[-0.04em] outline-none"
        >
          {t("updates.nextRelease.product")}
          <span className="mt-3 block text-[0.65em] font-medium tracking-[-0.03em] text-white/85">
            {t("updates.nextRelease.waiting")}
          </span>
        </motion.h1>
        <AnimatePresence>
          {!hello && (
            <motion.div
              initial={reducedMotion ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ ...reveal, delay: reducedMotion ? 0 : 0.35 }}
              className="mt-7 max-w-lg"
            >
              <p className="mx-auto max-w-xs text-sm leading-relaxed text-white/60 sm:max-w-md sm:text-base">
                {t("updates.nextRelease.description")}
              </p>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <footer className="relative flex min-h-44 shrink-0 items-end justify-center px-6 pb-[calc(env(safe-area-inset-bottom)+1.5rem)] sm:min-h-40 sm:pb-10">
        <AnimatePresence>
          {!hello && (
            <motion.div
              initial={reducedMotion ? false : { opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ ...reveal, delay: reducedMotion ? 0 : 0.6 }}
              className="flex w-full max-w-sm flex-col items-center gap-2 sm:max-w-none sm:flex-row sm:justify-center sm:gap-3"
            >
              <button
                type="button"
                onClick={() => void openExternalUrl(update?.downloadUrl ?? "https://www.lettuceai.app/download")}
                className="inline-flex min-h-12 w-full items-center justify-center gap-3 rounded-full bg-white px-7 py-3 text-sm font-semibold text-black transition hover:bg-white/90 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-white sm:w-auto"
              >
                {t("updates.nextRelease.action")}
                <ArrowUpRight aria-hidden className="h-4 w-4" />
              </button>
              <button
                type="button"
                onClick={() => navigate(returnTo, { replace: true })}
                className="min-h-12 w-full rounded-full px-6 py-3 text-sm font-medium text-white/70 transition hover:bg-white/8 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-white sm:w-auto"
              >
                {t("updates.nextRelease.later")}
              </button>
            </motion.div>
          )}
        </AnimatePresence>
      </footer>
    </section>
  );
}
