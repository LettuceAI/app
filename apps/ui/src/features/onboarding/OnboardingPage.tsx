import { useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn, IconButton, icon, motion as m, Surface, useCompactLayout } from "@lettuceai/crisp";
import { art, artTall } from "./assets";
import { ActionButton } from "./components/ui";
import { guideContent } from "./content/guide-content";
import { learnSlides } from "./content/slides";
import { applyManualMemory, completeOnboarding, enableAutomaticMemory, hasEidosInstalled } from "./data/completion";
import { canConnect, classifyConnectError, connectProvider } from "./data/connect";
import { useEmbeddingInstall } from "./data/install-job";
import { externalModelId, saveChosenModel } from "./data/models";
import { catalogQuery, useApi } from "./data/queries";
import { useAction } from "./data/use-action";
import { OnboardingContext, type OnboardingContextValue } from "./flow/context";
import { flowReducer, initialFlow, progressOf, type Where } from "./flow/flow";
import { ConnectScreen } from "./screens/ConnectScreen";
import { GuideScreen } from "./screens/GuideScreen";
import { KeyScreen } from "./screens/KeyScreen";
import { LearnScreen } from "./screens/LearnScreen";
import { MemoryScreen } from "./screens/MemoryScreen";
import { ModelScreen } from "./screens/ModelScreen";
import { PathScreen } from "./screens/PathScreen";
import { WelcomeScreen } from "./screens/WelcomeScreen";
import { MemoryDownloadSheet } from "./sheets/MemoryDownloadSheet";
import { OtherOptionsSheet } from "./sheets/OtherOptionsSheet";
import { SkipWarningSheet } from "./sheets/SkipWarningSheet";

const foregroundOnArt = "#ffffff";

const phoneVeil = "linear-gradient(to bottom, rgb(0 0 0 / 0) 0%, rgb(0 0 0 / 0.55) 26%, rgb(0 0 0 / 0.68) 100%)";
const desktopVeil = "linear-gradient(to bottom, rgb(0 0 0 / 0) 0%, rgb(0 0 0 / 0.5) 28%, rgb(0 0 0 / 0.66) 100%)";
const phoneMask = "linear-gradient(to bottom, black calc(100% - 8rem), transparent calc(100% - 4.5rem))";

interface Primary {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  onClick: () => void;
}

export function OnboardingPage() {
  const { t } = useTranslation("onboarding");
  const { i18n } = useTranslation();
  const api = useApi();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const phone = useCompactLayout();
  const [state, dispatch] = useReducer(flowReducer, initialFlow);
  const [other, setOther] = useState(false);
  const [otherKey, setOtherKey] = useState(0);
  const [skipWarning, setSkipWarning] = useState(false);
  const [memorySheet, setMemorySheet] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  const install = useEmbeddingInstall(api);
  const services = useMemo(() => ({ api, queryClient }), [api, queryClient]);

  const catalog = useQuery(catalogQuery(api));
  const guides = useMemo(
    () => ({
      gemini: guideContent[phone ? "geminiMobile" : "geminiDesktop"].length,
      openrouter: guideContent[phone ? "openrouterMobile" : "openrouterDesktop"].length,
    }),
    [phone],
  );

  const descriptor = catalog.data?.find((entry) => entry.kind === state.providerKind) ?? null;
  const routeName = state.route === "provider" ? (descriptor?.display_name ?? "") : t(`guide.routes.${state.route}`);

  const finishApp = async () => {
    await completeOnboarding(services);
    await navigate({ to: "/", replace: true });
  };

  const connect = useAction(async () => {
    if (!descriptor || !canConnect(descriptor, state)) return;
    const account = await connectProvider({
      api,
      queryClient,
      descriptor,
      flow: state,
      fallbackLabel: t("connect.form.namePlaceholder", { name: routeName }),
    });
    dispatch({ type: "patch", patch: { account } });
    dispatch({ type: "go", step: "model" });
  }, classifyConnectError);

  const saveModel = useAction(async () => {
    if (externalModelId(state) === "") return;
    const savedModel = await saveChosenModel({ api, queryClient, flow: state });
    dispatch({ type: "patch", patch: { savedModel } });
    dispatch({ type: "go", step: "memory" });
  });

  const finish = useAction(async () => {
    if (state.memory === "manual") {
      await applyManualMemory(services);
      await finishApp();
      return;
    }
    if (await hasEidosInstalled(services)) {
      await enableAutomaticMemory(services, state.savedModel?.id ?? null, true);
      await finishApp();
      return;
    }
    install.reset();
    setMemorySheet(true);
  });

  const openApp = useAction(async () => {
    await enableAutomaticMemory(services, state.savedModel?.id ?? null, false);
    await finishApp();
  });

  const skip = useAction(async () => {
    const models = await api.call("modelsList");
    if (models.models.length > 0) {
      await finishApp();
      return;
    }
    setOther(false);
    setSkipWarning(true);
  });

  const skipAnyway = useAction(finishApp);

  const changeLanguage = async (locale: string) => {
    await api.call("appUiStateUpdate", { patch: { locale } });
    await i18n.changeLanguage(locale);
  };

  const value: OnboardingContextValue = { state, dispatch, phone, guides, catalog, connect, saveModel, finish };
  const { step } = state;
  const guideActive = step === "gemini" || step === "openrouter";
  const onGuideKey = guideActive && state.guide === guides[step];
  const progress = progressOf(state, guides);
  const workbench = step === "connect";

  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [step, state.providerKind, state.guide]);

  const whereText = (where: Where): string => {
    switch (where?.kind) {
      case "learn":
        return t("header.learn", { current: where.current, total: where.total });
      case "path":
        return t("header.path");
      case "connect":
        return t("header.connect");
      case "guided":
        return t("header.guided", {
          name: where.route === "provider" ? routeName : t(`guide.routes.${where.route}`),
          current: where.current,
          total: where.total,
        });
      case "model":
        return t("header.model");
      case "memory":
        return t("header.memory");
      default:
        return "";
    }
  };

  const connectPrimary: Primary = {
    label: connect.busy ? t("actions.checking") : t("actions.connect"),
    loading: connect.busy,
    disabled: connect.busy || !canConnect(descriptor, state),
    onClick: connect.run,
  };
  const lastSlide = state.slide >= learnSlides.length - 1;

  const primary: Primary | null =
    step === "learn"
      ? {
          label: lastSlide ? t("actions.gotIt") : t("actions.next"),
          onClick: () => (lastSlide ? dispatch({ type: "go", step: "path" }) : dispatch({ type: "patch", patch: { slide: state.slide + 1 } })),
        }
      : guideActive && !onGuideKey
        ? {
            label: state.guide === guides[step] - 1 ? t("actions.haveKey") : t("actions.next"),
            onClick: () => dispatch({ type: "patch", patch: { guide: state.guide + 1, guideSlide: 0 } }),
          }
        : onGuideKey
          ? connectPrimary
          : step === "connect" && phone && !descriptor
            ? null
            : step === "connect"
              ? connectPrimary
              : step === "model" && !phone
                ? null
                : step === "model"
                  ? {
                      label: saveModel.busy ? t("actions.saving") : t("actions.useModel"),
                      disabled: saveModel.busy || externalModelId(state) === "",
                      loading: saveModel.busy,
                      onClick: saveModel.run,
                    }
                  : step === "memory" && !phone
                    ? null
                    : step === "memory"
                      ? {
                          label: finish.busy ? t("actions.finishing") : t("actions.finish"),
                          disabled: finish.busy,
                          loading: finish.busy,
                          onClick: finish.run,
                        }
                      : null;

  const screen: ReactNode =
    step === "welcome" ? (
      <WelcomeScreen
        onOtherOptions={() => {
          setOtherKey((key) => key + 1);
          setOther(true);
        }}
      />
    ) : step === "connect" ? (
      <ConnectScreen />
    ) : step === "learn" ? (
      <LearnScreen />
    ) : step === "path" ? (
      <PathScreen />
    ) : step === "model" ? (
      <ModelScreen />
    ) : step === "memory" ? (
      <MemoryScreen />
    ) : onGuideKey ? (
      <KeyScreen route={step} />
    ) : (
      <GuideScreen route={step} />
    );

  const spacer = phone
    ? workbench
      ? "1.5rem"
      : step === "model" || step === "memory"
        ? "14%"
        : "26%"
    : step === "connect"
      ? "3%"
      : step === "model" || step === "memory"
        ? "16%"
        : "22%";

  return (
    <OnboardingContext.Provider value={value}>
      <Surface fg={foregroundOnArt} tone="dark" className="flex h-full flex-col bg-black">
        <img src={phone ? artTall : art} alt="" className="pointer-events-none absolute inset-0 h-full w-full object-cover" />
        <motion.div
          aria-hidden
          className="pointer-events-none absolute inset-0"
          style={{ background: phone ? phoneVeil : desktopVeil }}
          initial={false}
          animate={{ opacity: step === "welcome" ? 0 : step === "learn" || step === "path" ? 0.6 : 1 }}
          transition={m.settle}
        />
        <div className={cn("relative z-10 flex h-14 shrink-0 items-center", phone ? "px-3" : "px-6")}>
          {step !== "welcome" && (
            <IconButton
              label={t("header.back")}
              variant="ghost"
              shape="round"
              onClick={() => dispatch({ type: "back", phone, guides })}
            >
              <ArrowLeft size={icon.lg} />
            </IconButton>
          )}
          <span className="flex-1" />
          {step !== "welcome" && <span className="text-sm text-fg-3">{whereText(progress.where)}</span>}
        </div>
        {phone ? (
          step !== "welcome" && (
            <div className="relative z-10 flex shrink-0 gap-1.5 px-5 pt-1">
              {Array.from({ length: progress.segments }, (_, index) => (
                <span
                  key={index}
                  className={cn("h-1 flex-1 rounded-full transition-colors motion-slow", index < progress.filled ? "bg-accent" : "bg-white/15")}
                />
              ))}
            </div>
          )
        ) : (
          <div className={cn("relative z-10 h-px shrink-0", step !== "welcome" && "bg-line")}>
            <motion.span
              className="block h-full bg-accent"
              initial={false}
              animate={{ width: `${progress.fraction * 100}%` }}
              transition={m.settle}
            />
          </div>
        )}
        <div
          ref={scroller}
          className={cn("relative z-10 flex min-h-0 flex-1 flex-col", phone ? "overflow-y-auto [scrollbar-width:none]" : "overflow-hidden")}
          style={phone && step !== "welcome" ? { maskImage: phoneMask, WebkitMaskImage: phoneMask } : undefined}
        >
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={`${step}-${state.guide}`}
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -12 }}
              transition={m.settle}
              className="flex min-h-0 flex-1 flex-col"
            >
              <div className="shrink-0" style={{ height: spacer }} />
              <div
                className={cn(
                  "mx-auto flex w-full min-h-0 flex-1 flex-col",
                  phone ? "px-5 pb-12" : workbench ? "max-w-6xl px-10 pb-8" : "max-w-5xl px-10 pb-5",
                )}
              >
                <div className={cn("flex min-h-0 flex-1 flex-col", !phone && "overflow-y-auto [scrollbar-width:none]")}>
                  {screen}
                  {phone && step !== "welcome" && <div aria-hidden className="h-28 shrink-0" />}
                </div>
                {!phone && !workbench && (
                  <div className="flex h-16 shrink-0 items-end gap-3">
                    <span className="flex-1" />
                    {primary && <ActionButton label={primary.label} disabled={primary.disabled ?? false} loading={primary.loading ?? false} onClick={primary.onClick} />}
                  </div>
                )}
              </div>
            </motion.div>
          </AnimatePresence>
        </div>
        <AnimatePresence>
          {phone && step !== "welcome" && primary && (
            <motion.div
              key="phone-action"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0, transition: { ...m.settle, delay: 0.12 } }}
              exit={{ opacity: 0, y: 16 }}
              transition={m.settle}
              className="absolute inset-x-0 bottom-0 z-20 flex items-center gap-2 px-5 pt-3 pb-[calc(var(--safe-bottom,0px)+1.25rem)]"
            >
              <ActionButton label={primary.label} disabled={primary.disabled ?? false} loading={primary.loading ?? false} block onClick={primary.onClick} />
            </motion.div>
          )}
        </AnimatePresence>

        <OtherOptionsSheet
          key={otherKey}
          open={other}
          onClose={() => setOther(false)}
          onSkip={skip.run}
          onLanguage={changeLanguage}
          skipping={skip.busy}
          skipError={skip.error}
        />
        <SkipWarningSheet
          open={skipWarning}
          onClose={() => setSkipWarning(false)}
          onConnect={() => {
            setSkipWarning(false);
            dispatch({ type: "go", step: "connect" });
          }}
          onSkip={skipAnyway.run}
          skipping={skipAnyway.busy}
          error={skipAnyway.error}
        />
        <MemoryDownloadSheet
          open={memorySheet}
          onClose={() => setMemorySheet(false)}
          install={install.state}
          onStart={() => void install.start()}
          onManual={() => {
            dispatch({ type: "patch", patch: { memory: "manual" } });
            setMemorySheet(false);
          }}
          onOpenApp={openApp.run}
          opening={openApp.busy}
          openError={openApp.error}
        />
      </Surface>
    </OnboardingContext.Provider>
  );
}
