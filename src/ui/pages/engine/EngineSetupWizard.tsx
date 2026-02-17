import { useParams } from "react-router-dom";
import { Check } from "lucide-react";
import { useEffect, useState } from "react";

import { useEngineSetupController } from "./hooks/useEngineSetupController";
import { WelcomeStep } from "./steps/WelcomeStep";
import { ProvidersStep } from "./steps/ProvidersStep";
import { SettingsStep } from "./steps/SettingsStep";
import { readSettings } from "../../../core/storage/repo";
import { useNavigationManager } from "../../navigation";
import type { ProviderCredential } from "../../../core/storage/schemas";

const STEP_ORDER = ["welcome", "providers", "settings", "done"] as const;

function StepIndicator({ current }: { current: number }) {
  return (
    <div className="flex items-center justify-center gap-2 py-4">
      {STEP_ORDER.slice(0, -1).map((_, i) => (
        <div
          key={i}
          className={`h-1.5 rounded-full transition-all ${
            i < current
              ? "w-6 bg-emerald-400"
              : i === current
                ? "w-6 bg-white/60"
                : "w-3 bg-white/20"
          }`}
        />
      ))}
    </div>
  );
}

export function EngineSetupWizard() {
  const { credentialId } = useParams<{ credentialId: string }>();
  const { back } = useNavigationManager();
  const [credential, setCredential] = useState<ProviderCredential | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const settings = await readSettings();
        const cred = settings.providerCredentials.find((p) => p.id === credentialId);
        if (!cancelled && cred) {
          setCredential(cred);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [credentialId]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (!credential) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4">
        <p className="text-sm text-white/60">Engine provider not found.</p>
        <button
          onClick={back}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/70 hover:bg-white/10"
        >
          Go Back
        </button>
      </div>
    );
  }

  return <WizardInner credential={credential} />;
}

function WizardInner({ credential }: { credential: ProviderCredential }) {
  const baseUrl = credential.baseUrl || "";
  const apiKey = credential.apiKey || "";

  const {
    state,
    appProviders,
    setStep,
    setDefaultBackend,
    updateLlmProvider,
    updateEngineSettings,
    importAppProvider,
    saveProviders,
    saveSettings,
    finishWizard,
  } = useEngineSetupController(baseUrl, apiKey, credential.id);

  const currentIndex = STEP_ORDER.indexOf(state.step);

  return (
    <div className="flex h-full flex-col">
      <StepIndicator current={currentIndex} />

      {/* Content */}
      <div className="flex-1 overflow-y-auto pb-[env(safe-area-inset-bottom)]">
        {state.step === "welcome" && <WelcomeStep onNext={() => setStep("providers")} />}

        {state.step === "providers" && (
          <ProvidersStep
            llmProviders={state.llmProviders}
            defaultBackend={state.defaultBackend}
            isSaving={state.isSaving}
            error={state.error}
            appProviders={appProviders}
            onUpdate={updateLlmProvider}
            onSetDefault={setDefaultBackend}
            onImport={importAppProvider}
            onSave={saveProviders}
            onNext={() => setStep("settings")}
          />
        )}

        {state.step === "settings" && (
          <SettingsStep
            settings={state.engineSettings}
            isSaving={state.isSaving}
            error={state.error}
            onUpdate={updateEngineSettings}
            onSave={saveSettings}
          />
        )}

        {state.step === "done" && (
          <div className="flex flex-col items-center px-4 py-12">
            <div className="flex h-20 w-20 items-center justify-center rounded-3xl border border-emerald-400/30 bg-emerald-500/15">
              <Check className="h-10 w-10 text-emerald-300" />
            </div>
            <h2 className="mt-6 text-xl font-bold text-white">Setup Complete!</h2>
            <p className="mt-2 text-center text-sm text-white/60">
              Your Lettuce Engine is configured and ready to go.
            </p>
            <button
              onClick={finishWizard}
              className="mt-8 rounded-full border border-emerald-400/40 bg-emerald-500/20 px-8 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 active:scale-[0.98]"
            >
              Open Dashboard
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
