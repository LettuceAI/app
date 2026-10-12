import { useEffect, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowRight, Check, Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, fileSize, icon, motion as m, Surface, useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";
import type { RequiredModel } from "@/api/generated/bindings";
import { lettuce } from "./assets";
import { Backdrop } from "./components/Backdrop";
import { StatusList, type StatusRow } from "./components/StatusList";
import { finishedStages, migrationStages, useLegacyMigration } from "./data/migration";
import { modelsNeededAfterMove, useModelInstalls, type ModelInstall } from "./data/models";

type Step = "welcome" | "migrate" | "models" | "done" | "upgrade";

const releasesUrl = "https://github.com/LettuceAI/app/releases";
const helloMs = 2200;
const ease = [0.22, 1, 0.36, 1] as const;
const ring = 2 * Math.PI * 54;
const pill = "min-w-56 rounded-full bg-white text-black hover:bg-white/90";

const noModels: readonly RequiredModel[] = [];

export function WelcomeBackPage() {
  const { t, i18n } = useTranslation("welcome-back");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const phone = useCompactLayout();
  const status = useQuery(appStatusQuery(api));
  const legacy = status.data?.legacy_state;
  const [chosen, setStep] = useState<Exclude<Step, "upgrade">>("welcome");
  const step: Step = legacy?.type === "requires_legacy_upgrade" ? "upgrade" : chosen;
  const [hello, setHello] = useState(true);
  const [needed, setNeeded] = useState<readonly RequiredModel[]>(noModels);
  const [copied, setCopied] = useState(false);
  const [leaving, setLeaving] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setHello(false), helloMs);
    return () => window.clearTimeout(timer);
  }, []);

  const migration = useLegacyMigration(api, async () => {
    await api.call("appUiStateUpdate", { patch: { legacyMigration: "migrated" } });
    await queryClient.invalidateQueries();
    const missing = await modelsNeededAfterMove(api);
    setNeeded(missing);
    setStep(missing.length > 0 ? "models" : "done");
  });
  const installs = useModelInstalls(api, needed);
  const allInstalled = installs.installs.length > 0 && installs.installs.every((entry) => entry.phase === "done");
  const anyFailed = installs.installs.some((entry) => entry.phase === "failed");
  const started = installs.running || installs.installs.some((entry) => entry.phase !== "queued");

  const finish = async () => {
    setLeaving(true);
    try {
      const current = await queryClient.fetchQuery(appStatusQuery(api));
      await api.call("appUiStateUpdate", { patch: { onboarding: { completed: true }, lastSeenAppVersion: current.version } });
      await queryClient.invalidateQueries({ queryKey: queryKeys.app.status() });
      await navigate({ to: "/", replace: true });
    } finally {
      setLeaving(false);
    }
  };

  const decline = async () => {
    setLeaving(true);
    try {
      await api.call("appUiStateUpdate", { patch: { legacyMigration: "declined" } });
      await queryClient.invalidateQueries({ queryKey: queryKeys.app.status() });
      await navigate({ to: "/welcome", replace: true });
    } finally {
      setLeaving(false);
    }
  };

  const copyLink = async () => {
    await navigator.clipboard.writeText(releasesUrl);
    setCopied(true);
  };

  const begin = () => {
    setStep("migrate");
    void migration.start();
  };

  const kinds = needed.length > 1 ? "both" : needed[0] === "emotion" ? "emotion" : "memory";
  const finished = finishedStages(migration.state.job);
  const failed = migration.state.phase === "failed";

  const center = (children: ReactNode) => (
    <div className={cn("mx-auto flex w-full flex-1 flex-col items-center justify-center text-center", phone ? "px-6" : "max-w-3xl px-10")}>{children}</div>
  );
  const title = (text: string, size: "hero" | "page" = "page") => (
    <h1
      className={cn(
        "font-bold tracking-[-0.03em] text-fg",
        size === "hero" ? (phone ? "text-[2.4rem] leading-[1.05]" : "text-6xl") : phone ? "text-3xl" : "text-5xl",
      )}
    >
      {text}
    </h1>
  );

  const stageRows: StatusRow[] = migrationStages.map((stage, index) => ({
    id: stage,
    title: t(`migrate.stages.${stage}`),
    status: index < finished ? "done" : index === finished && !failed ? "active" : "waiting",
  }));

  const modelDetail = (entry: ModelInstall): ReactNode => {
    const progress = entry.job?.progress;
    if (entry.phase === "done") return t("models.ready");
    if (entry.phase === "failed") return t("models.failed");
    if (entry.phase === "verifying") return t("models.checkingFiles");
    if (entry.phase === "downloading" && progress && progress.unit === "bytes" && progress.total !== null) {
      return t("models.progress", { done: fileSize(progress.current, i18n.language), total: fileSize(progress.total, i18n.language) });
    }
    if (started) return t("models.waiting");
    return t(`models.why.${entry.model}`);
  };
  const modelRows: StatusRow[] = installs.installs.map((entry) => {
    const progress = entry.job?.progress;
    const fraction = progress && progress.total ? progress.current / progress.total : 0;
    return {
      id: entry.model,
      title: t(`models.names.${entry.model}`),
      detail: modelDetail(entry),
      status:
        entry.phase === "done" ? "done"
        : entry.phase === "verifying" ? "active"
        : entry.phase === "downloading" ? { progress: fraction }
        : "waiting",
    };
  });

  const screens: Record<Step, ReactNode> = {
    welcome: center(
      <>
        <motion.h1
          layout
          initial={{ opacity: 0, filter: "blur(12px)", scale: 0.96 }}
          animate={{ opacity: 1, filter: "blur(0px)", scale: 1 }}
          transition={{ duration: 1.2, ease, layout: { duration: 0.9, ease } }}
          className={cn("whitespace-nowrap font-bold tracking-[-0.04em] text-fg", phone ? "text-[3rem] leading-[1.05]" : "text-8xl leading-[1]")}
        >
          {t("welcome.title")}
        </motion.h1>
        <AnimatePresence>
          {!hello && (
            <motion.p
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.8, delay: 0.35, ease }}
              className={cn("mt-5 max-w-md text-fg-2", phone ? "text-lg" : "text-xl")}
            >
              {t("welcome.bodyBefore")}
              <span className="font-semibold text-fg">{t("welcome.product")}</span>
              {t("welcome.bodyAfter")}
            </motion.p>
          )}
        </AnimatePresence>
      </>,
    ),
    migrate: center(
      <>
        <div className={cn("relative", phone ? "h-24 w-24" : "h-28 w-28")}>
          <svg viewBox="0 0 120 120" className="absolute inset-0 h-full w-full -rotate-90">
            <circle cx="60" cy="60" r="54" fill="none" stroke="rgb(255 255 255 / 0.1)" strokeWidth="3" />
            <motion.circle
              cx="60"
              cy="60"
              r="54"
              fill="none"
              stroke="#ffffff"
              strokeWidth="3"
              strokeLinecap="round"
              strokeDasharray={ring}
              initial={false}
              animate={{ strokeDashoffset: ring * (1 - Math.min(finished + (failed ? 0 : 0.5), migrationStages.length) / migrationStages.length) }}
              transition={{ duration: 1.2, ease }}
            />
          </svg>
          <img src={lettuce} alt="" className="absolute inset-0 m-auto h-10 w-10" />
        </div>
        <div className="mt-8">{title(t("migrate.title"))}</div>
        <p className="mt-3 text-lg text-fg-2">{t("migrate.body")}</p>
        <div className="mt-8 flex w-full justify-center">
          <StatusList rows={stageRows} />
        </div>
        {failed && (
          <p role="alert" className="mt-5 text-base text-danger">
            {t("migrate.failed")}
          </p>
        )}
      </>,
    ),
    models: center(
      <>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div key={allInstalled ? "ready" : "pending"} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -8 }} transition={m.settle}>
            {title(allInstalled ? t("models.titleReady") : t("models.titlePending"))}
            <p className="mx-auto mt-3 max-w-md text-lg text-fg-2">{allInstalled ? t(`models.bodyReady_${kinds}`) : t(`models.bodyPending_${kinds}`)}</p>
          </motion.div>
        </AnimatePresence>
        <div className="mt-9 flex w-full justify-center">
          <StatusList rows={modelRows} />
        </div>
        <p className="mt-6 min-h-5 text-sm text-fg-3">{allInstalled ? null : installs.running ? t("models.keepOpen") : started ? null : t("models.notNowNote")}</p>
      </>,
    ),
    done: center(
      <>
        <span className="relative flex h-16 w-16 items-center justify-center">
          {[0, 0.25].map((delay) => (
            <motion.span
              key={delay}
              aria-hidden
              className="absolute inset-0 rounded-full border border-white/60"
              initial={{ scale: 1, opacity: 0.8 }}
              animate={{ scale: 9, opacity: 0 }}
              transition={{ duration: 1.8, delay: 0.25 + delay, ease }}
            />
          ))}
          <motion.span
            className="relative flex h-16 w-16 items-center justify-center rounded-full bg-white text-black"
            initial={{ scale: 0.6, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ type: "spring", stiffness: 380, damping: 22 }}
          >
            <Check size={32} strokeWidth={2.5} />
          </motion.span>
        </span>
        <div className="mt-7">{title(t("done.title"), "hero")}</div>
        <p className="mt-4 text-lg text-fg-2">{t("done.body")}</p>
      </>,
    ),
    upgrade: center(
      <>
        <img src={lettuce} alt="" className="h-14 w-14" />
        <div className="mt-7">{title(t("upgrade.title"))}</div>
        <p className="mt-4 max-w-md text-lg leading-relaxed text-fg-2">
          {t("upgrade.body", { version: legacy?.type === "requires_legacy_upgrade" ? legacy.required_app_version : "" })}
        </p>
        <p className="mt-4 select-all text-sm text-fg-3">{releasesUrl}</p>
      </>,
    ),
  };

  const primary = (label: string, onClick: () => void, options: { leading?: ReactNode; trailing?: boolean; disabled?: boolean } = {}) => (
    <Button
      variant="primary"
      size="lg"
      block={phone}
      className={pill}
      disabled={options.disabled ?? false}
      leading={options.leading}
      trailing={options.trailing === false ? undefined : <ArrowRight size={icon.md} />}
      onClick={onClick}
    >
      {label}
    </Button>
  );
  const secondary = (label: string, onClick: () => void) => (
    <Button variant="ghost" size="lg" block={phone} className="rounded-full text-fg-2" disabled={leaving} onClick={onClick}>
      {label}
    </Button>
  );

  const actions: ReactNode =
    step === "welcome"
      ? hello
        ? null
        : (
          <>
            {primary(t("welcome.migrate"), begin)}
            {secondary(t("welcome.notNow"), () => void decline())}
          </>
        )
      : step === "migrate"
        ? failed
          ? primary(t("migrate.retry"), () => void migration.start())
          : <p className="text-sm text-fg-3">{t("migrate.note")}</p>
        : step === "models"
          ? allInstalled
            ? primary(t("models.continue"), () => setStep("done"))
            : installs.running
              ? null
              : (
                <>
                  {primary(anyFailed ? t("models.retry") : t("models.download"), () => void installs.start(), { leading: <Download size={icon.md} />, trailing: false })}
                  {secondary(t("models.notNow"), () => setStep("done"))}
                </>
              )
          : step === "done"
            ? primary(t("done.open"), () => void finish(), { disabled: leaving })
            : (
              <>
                {primary(copied ? t("upgrade.copied") : t("upgrade.copy"), () => void copyLink(), { trailing: false })}
                {secondary(t("upgrade.fresh"), () => void decline())}
              </>
            );

  const actionKey = `${step}-${hello}-${failed}-${installs.running}-${allInstalled}-${anyFailed}-${copied}`;

  return (
    <Surface fg="#ffffff" tone="dark" className="flex h-full flex-col bg-black">
      <Backdrop finished={step === "done"} dim={step === "migrate" || step === "models"} />
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={step}
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 1.02 }}
          transition={m.settle}
          className="relative z-10 flex min-h-0 flex-1 flex-col overflow-y-auto [scrollbar-width:none]"
        >
          {screens[step]}
        </motion.div>
      </AnimatePresence>
      <div className={cn("relative z-10 flex shrink-0 items-center justify-center", phone ? "min-h-36 flex-col gap-2 px-6 pb-[calc(var(--safe-bottom,0px)+1.5rem)]" : "h-36 gap-3 pb-10")}>
        <AnimatePresence mode="wait" initial={false}>
          {actions && (
            <motion.div
              key={actionKey}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0, transition: { ...m.settle, delay: step === "welcome" ? 0.6 : 0.2 } }}
              exit={{ opacity: 0 }}
              className={cn("flex flex-col items-center gap-2", phone && "w-full")}
            >
              {actions}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </Surface>
  );
}
