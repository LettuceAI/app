import { ArrowLeft, Loader } from "lucide-react";
import { useOnboardingController, OnboardingStep } from "./hooks/useOnboardingController";
import { ProviderStep } from "./steps/ProviderStep";
import { ModelStep } from "./steps/ModelStep";
import { MemoryStep } from "./steps/MemoryStep";
import { IntroStep } from "./steps/IntroStep";
import { LearnStep } from "./steps/LearnStep";
import { PathStep } from "./steps/PathStep";
import { GeminiSetupStep } from "./steps/GeminiSetupStep";
import { OpenRouterSetupStep } from "./steps/OpenRouterSetupStep";
import { MemorySetupStep } from "./steps/MemorySetupStep";
import { ModelRecommendations } from "./ModelRecommendations";
import { WelcomePage } from "./Welcome";
import { OnboardingSyncStep } from "./OnboardingSyncPage";
import welcomeBg from "../../../assets/welcomebackground.png";
import welcomeBgMobile from "../../../assets/welcomebackground-mobile.png";
import { cn, typography } from "../../design-tokens";
import { getPlatform } from "../../../core/utils/platform";
import { useState, useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { checkEmbeddingModel } from "../../../core/storage/repo";
import { setProviderSetupCompleted } from "../../../core/storage/appState";
import { useI18n } from "../../../core/i18n/context";

export function OnboardingPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const platform = getPlatform();
  const isDesktop = platform.type === "desktop";
  const controller = useOnboardingController();
  const { state } = controller;


  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "L") {
        e.preventDefault();
        navigate("/settings/logs");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate]);

  const [showDownloadModal, setShowDownloadModal] = useState(false);
  const [showRecommendations, setShowRecommendations] = useState(false);

  const showStepCounter =
    state.step === OnboardingStep.Provider ||
    state.step === OnboardingStep.Model ||
    state.step === OnboardingStep.Memory;

  const stepLabel =
    state.step === OnboardingStep.Provider
      ? t("onboarding.steps.provider")
      : state.step === OnboardingStep.Model
        ? t("onboarding.steps.model")
        : state.step === OnboardingStep.Memory
          ? t("onboarding.steps.memory")
          : state.step === OnboardingStep.GeminiSetup
            ? t("onboarding.gemini.kicker")
            : state.step === OnboardingStep.OpenRouterSetup
              ? t("onboarding.openrouter.kicker")
              : state.step === OnboardingStep.MemorySetup
                ? t("onboarding.memorySetup.kicker")
                : t("onboarding.steps.gettingStarted");

  const stepNumber =
    state.step === OnboardingStep.Provider ? 1 : state.step === OnboardingStep.Model ? 2 : 3;

  const goToStep = useCallback(
    (step: OnboardingStep, path: string) => {
      controller.setStep(step);
      window.history.replaceState(null, "", path);
    },
    [controller],
  );

  const handleChoosePath = useCallback(
    (choice: "free" | "paid") => {
      if (choice === "free") {
        goToStep(OnboardingStep.GeminiSetup, "/onboarding/gemini");
      } else {
        goToStep(OnboardingStep.OpenRouterSetup, "/onboarding/openrouter");
      }
    },
    [goToStep],
  );

  const handleBrowseModelLibrary = useCallback(async () => {
    await setProviderSetupCompleted(true);
    navigate("/settings/models/browse?returnTo=/onboarding/memory");
  }, [navigate]);

  const handleUseOwnGguf = useCallback(async () => {
    await setProviderSetupCompleted(true);
    navigate("/settings/models/new?provider=llamacpp&returnTo=/onboarding/memory");
  }, [navigate]);

  const handleFinish = useCallback(async () => {
    if (!state.memoryType) return;

    if (state.memoryType === "dynamic") {
      try {
        const modelExists = await checkEmbeddingModel();
        if (modelExists) {
          await controller.handleFinish();
        } else {
          setShowDownloadModal(true);
        }
      } catch (error) {
        console.error("Failed to check embedding model:", error);
        setShowDownloadModal(true);
      }
    } else {
      await controller.handleFinish();
    }
  }, [state.memoryType, controller]);

  const handleConfirmDownload = useCallback(() => {
    setShowDownloadModal(false);
    navigate("/settings/embedding-download?returnTo=/chat?firstTime=true");
  }, [navigate]);

  const handleSkipDownload = useCallback(async () => {
    setShowDownloadModal(false);
    controller.handleSelectMemoryType("manual");
    await controller.handleFinish();
  }, [controller]);

  if (state.capabilitiesLoading && state.step === OnboardingStep.Provider) {
    return (
      <div className="flex min-h-full flex-1 flex-col items-center justify-center bg-surface text-gray-200">
        <div className="flex items-center gap-3">
          <Loader size={20} className="animate-spin" />
          <span>{t("onboarding.loading")}</span>
        </div>
      </div>
    );
  }

  const isWelcomeStep = state.step === OnboardingStep.Welcome;
  const isSyncStep = state.step === OnboardingStep.Sync;
  const isStandaloneStep = isWelcomeStep || isSyncStep;

  return (
    <div className="relative flex h-[calc(100dvh-var(--titlebar-h,0px))] flex-col text-gray-200">
      {/* Background image + overlay */}
      <div
        aria-hidden="true"
        className="pointer-events-none fixed inset-0 bg-cover bg-center bg-no-repeat lg:hidden"
        style={{ backgroundImage: `url(${welcomeBgMobile})` }}
      />
      <div
        aria-hidden="true"
        className="pointer-events-none fixed inset-0 hidden bg-cover bg-center bg-no-repeat lg:block"
        style={{ backgroundImage: `url(${welcomeBg})` }}
      />
      <div
        aria-hidden="true"
        className="pointer-events-none fixed inset-0 bg-[linear-gradient(180deg,rgba(5,5,5,0.32)_0%,rgba(5,5,5,0.42)_55%,rgba(5,5,5,0.62)_100%)]"
      />

      {/* Header */}
      <div
        className={cn(
          "fixed top-[var(--titlebar-h,0px)] left-0 right-0 z-30 flex items-center justify-between",
          "bg-[linear-gradient(180deg,rgba(5,5,5,0.7)_0%,rgba(5,5,5,0.4)_70%,rgba(5,5,5,0)_100%)]",
          isDesktop ? "px-8 py-6 pb-10" : "px-4 py-4 pb-8 pt-[calc(env(safe-area-inset-top)+16px)]",
          isStandaloneStep && "hidden",
        )}
      >
        <button
          onClick={controller.goBack}
          className={cn(
            "flex items-center justify-center rounded-full border border-white/15 bg-white/8 text-white transition-all duration-200 hover:border-white/30 hover:bg-white/15 active:scale-[0.98]",
            isDesktop ? "w-11 h-11" : "w-10 h-10",
          )}
        >
          <ArrowLeft size={isDesktop ? 18 : 16} />
        </button>
        <div className="text-center">
          {showStepCounter && (
            <p
              className={cn(
                typography.caption.size,
                "font-medium uppercase tracking-[0.25em] text-white/55",
              )}
            >
              {t("onboarding.stepIndicator", { current: stepNumber, total: 3 })}
            </p>
          )}
          <p className="text-[13px] text-white/70 mt-0.5">{stepLabel}</p>
        </div>
        <div className={isDesktop ? "w-11" : "w-10"} />
      </div>

      {/* Main Content */}
      <main
        className={cn(
          "relative z-10 flex flex-1 flex-col overflow-y-auto",
          !isStandaloneStep &&
            (isDesktop
              ? "pt-[88px] pb-6"
              : "pt-[calc(env(safe-area-inset-top)+92px)] px-4 pb-[calc(env(safe-area-inset-bottom)+16px)]"),
        )}
      >
        {showRecommendations ? (
          <ModelRecommendations onBack={() => setShowRecommendations(false)} />
        ) : (
          <>
            {state.step === OnboardingStep.Welcome && (
              <div key="welcome" className="flex flex-1 flex-col">
                <WelcomePage
                  onContinue={() => {
                    controller.setStep(OnboardingStep.Intro);
                    window.history.replaceState(null, "", "/onboarding/start");
                  }}
                  onGoToSync={() => {
                    controller.setStep(OnboardingStep.Sync);
                    window.history.replaceState(null, "", "/onboarding/sync");
                  }}
                />
              </div>
            )}

            {state.step === OnboardingStep.Sync && (
              <div key="sync" className="flex flex-1 flex-col">
                <OnboardingSyncStep />
              </div>
            )}

            {state.step === OnboardingStep.Intro && (
              <div key="intro" className="flex flex-1 flex-col">
                <IntroStep
                  onFirstTime={() => goToStep(OnboardingStep.Learn, "/onboarding/learn")}
                  onExperienced={() =>
                    goToStep(OnboardingStep.Provider, "/onboarding/provider")
                  }
                />
              </div>
            )}

            {state.step === OnboardingStep.Learn && (
              <div key="learn" className="flex flex-1 flex-col">
                <LearnStep
                  onDone={() => goToStep(OnboardingStep.Path, "/onboarding/path")}
                  onExitBack={() => goToStep(OnboardingStep.Intro, "/onboarding/start")}
                />
              </div>
            )}

            {state.step === OnboardingStep.Path && (
              <div key="path" className="flex flex-1 flex-col">
                <PathStep onChoose={handleChoosePath} />
              </div>
            )}

            {state.step === OnboardingStep.GeminiSetup && (
              <div key="gemini" className="flex flex-1 flex-col">
                <GeminiSetupStep
                  onExitBack={() => goToStep(OnboardingStep.Path, "/onboarding/path")}
                />
              </div>
            )}

            {state.step === OnboardingStep.OpenRouterSetup && (
              <div key="openrouter" className="flex flex-1 flex-col">
                <OpenRouterSetupStep
                  onExitBack={() => goToStep(OnboardingStep.Path, "/onboarding/path")}
                />
              </div>
            )}

            {state.step === OnboardingStep.MemorySetup && (
              <div key="memory-setup" className="flex flex-1 flex-col">
                <MemorySetupStep />
              </div>
            )}

            {state.step === OnboardingStep.Provider && (
              <div key="provider" className="flex flex-1 flex-col">
                <ProviderStep
                  capabilities={state.capabilities}
                  selectedProviderId={state.selectedProviderId}
                  label={state.providerLabel}
                  apiKey={state.apiKey}
                  baseUrl={state.baseUrl}
                  config={state.config}
                  testResult={state.testResult}
                  isTesting={state.isTesting}
                  isSubmitting={state.isSubmittingProvider}
                  canTest={controller.canTestProvider}
                  canSave={controller.canSaveProvider}
                  onSelectProvider={controller.handleSelectProvider}
                  onLabelChange={controller.handleProviderLabelChange}
                  onApiKeyChange={controller.handleApiKeyChange}
                  onBaseUrlChange={controller.handleBaseUrlChange}
                  onConfigChange={controller.handleConfigChange}
                  onTestConnection={controller.handleTestConnection}
                  onSave={controller.handleSaveProvider}
                  onBrowseModelLibrary={() => void handleBrowseModelLibrary()}
                  onUseOwnGguf={() => void handleUseOwnGguf()}
                />
              </div>
            )}

            {state.step === OnboardingStep.Model && (
              <div key="model" className="flex flex-1 flex-col">
                <ModelStep
                  providers={state.providerCredentials}
                  selectedCredential={state.selectedCredential}
                  modelName={state.modelName}
                  displayName={state.displayName}
                  error={state.modelError}
                  isLoading={state.modelLoading}
                  isSaving={state.isSavingModel}
                  canSave={controller.canSaveModel}
                  onSelectCredential={controller.handleSelectCredential}
                  onModelNameChange={controller.handleModelNameChange}
                  onDisplayNameChange={controller.handleDisplayNameChange}
                  onSave={controller.handleSaveModel}
                  onSkip={controller.handleSkipModel}
                  onGoBack={controller.goBack}
                />
              </div>
            )}

            {state.step === OnboardingStep.Memory && (
              <div key="memory" className="flex flex-1 flex-col">
                <MemoryStep
                  selectedType={state.memoryType}
                  isProcessing={state.isProcessingMemory}
                  showDownloadModal={showDownloadModal}
                  onSelectType={controller.handleSelectMemoryType}
                  onFinish={handleFinish}
                  onCloseModal={() => setShowDownloadModal(false)}
                  onConfirmDownload={handleConfirmDownload}
                  onSkipDownload={handleSkipDownload}
                />
              </div>
            )}
          </>
        )}
      </main>
    </div>
  );
}
