import { useParams } from "react-router-dom";
import { useEffect, useState } from "react";

import { useEngineCharacterController } from "./hooks/useEngineCharacterController";
import { STEP_ORDER } from "./hooks/engineCharacterReducer";
import { CharacterModeStep } from "./steps/CharacterModeStep";
import { CharacterIdentityStep } from "./steps/CharacterIdentityStep";
import { CharacterPersonalityStep } from "./steps/CharacterPersonalityStep";
import { CharacterWorldStep } from "./steps/CharacterWorldStep";
import { CharacterReviewStep } from "./steps/CharacterReviewStep";
import { readSettings } from "../../../core/storage/repo";
import { useNavigationManager } from "../../navigation";
import type { ProviderCredential } from "../../../core/storage/schemas";

function StepIndicator({ current }: { current: number }) {
  return (
    <div className="flex items-center justify-center gap-2 py-4">
      {STEP_ORDER.map((_, i) => (
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

export function EngineCharacterCreate() {
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
        if (!cancelled && cred) setCredential(cred);
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

  return <CreateInner credential={credential} />;
}

function CreateInner({ credential }: { credential: ProviderCredential }) {
  const baseUrl = credential.baseUrl || "";
  const apiKey = credential.apiKey || "";
  const {
    state,
    setStep,
    setBoostField,
    setField,
    setSpeechPattern,
    setTimeBehavior,
    setEmotion,
    boost,
    save,
  } = useEngineCharacterController(baseUrl, apiKey, credential.id);

  const currentIndex = STEP_ORDER.indexOf(state.step);

  return (
    <div className="flex h-full flex-col">
      <StepIndicator current={currentIndex} />

      <div className="flex-1 overflow-y-auto pb-[env(safe-area-inset-bottom)]">
        {state.step === "mode" && (
          <CharacterModeStep
            boostName={state.boostName}
            boostSeed={state.boostSeed}
            boostEra={state.boostEra}
            boosting={state.boosting}
            boostError={state.boostError}
            onBoostFieldChange={setBoostField}
            onBoost={() => void boost()}
            onManual={() => setStep("identity")}
          />
        )}

        {state.step === "identity" && (
          <CharacterIdentityStep
            name={state.name}
            era={state.era}
            role={state.role}
            setting={state.setting}
            coreIdentity={state.coreIdentity}
            backstory={state.backstory}
            boosted={state.boosted}
            onFieldChange={setField}
            onNext={() => setStep("personality")}
          />
        )}

        {state.step === "personality" && (
          <CharacterPersonalityStep
            personalityTraits={state.personalityTraits}
            speechPatterns={state.speechPatterns}
            onFieldChange={setField}
            onSpeechPatternChange={setSpeechPattern}
            onNext={() => setStep("world")}
          />
        )}

        {state.step === "world" && (
          <CharacterWorldStep
            knowledgeDomains={state.knowledgeDomains}
            knowledgeBoundaries={state.knowledgeBoundaries}
            researchSeeds={state.researchSeeds}
            researchEnabled={state.researchEnabled}
            physicalDescription={state.physicalDescription}
            physicalHabits={state.physicalHabits}
            idleBehaviors={state.idleBehaviors}
            timeBehaviors={state.timeBehaviors}
            baselineEmotions={state.baselineEmotions}
            backend={state.backend}
            model={state.model}
            temperature={state.temperature}
            onFieldChange={setField}
            onTimeBehaviorChange={setTimeBehavior}
            onEmotionChange={setEmotion}
            onNext={() => setStep("review")}
          />
        )}

        {state.step === "review" && (
          <CharacterReviewStep
            name={state.name}
            era={state.era}
            role={state.role}
            setting={state.setting}
            coreIdentity={state.coreIdentity}
            backstory={state.backstory}
            personalityTraits={state.personalityTraits}
            speechPatterns={state.speechPatterns}
            knowledgeDomains={state.knowledgeDomains}
            knowledgeBoundaries={state.knowledgeBoundaries}
            researchSeeds={state.researchSeeds}
            researchEnabled={state.researchEnabled}
            physicalDescription={state.physicalDescription}
            physicalHabits={state.physicalHabits}
            idleBehaviors={state.idleBehaviors}
            timeBehaviors={state.timeBehaviors}
            baselineEmotions={state.baselineEmotions}
            backend={state.backend}
            model={state.model}
            temperature={state.temperature}
            saving={state.saving}
            error={state.error}
            onSave={() => void save()}
            onEdit={setStep}
          />
        )}
      </div>
    </div>
  );
}
