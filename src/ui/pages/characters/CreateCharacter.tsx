import { useNavigate, useLocation } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import React from "react";

import { useCharacterForm, Step } from "./hooks/useCharacterForm";
//import { ProgressIndicator } from "./components/ProgressIndicator";
import { IdentityStep } from "./components/IdentityStep";
import { StartingSceneStep } from "./components/StartingSceneStep";
import { DescriptionStep } from "./components/DescriptionStep";
import { ExtrasStep } from "./components/ExtrasStep";
import { TopNav } from "../../components/App";
import {
  listAudioProviders,
  listUserVoices,
  getProviderVoices,
  refreshProviderVoices,
  type AudioProvider,
  type CachedVoice,
  type UserVoice,
} from "../../../core/storage/audioProviders";

export function CreateCharacterPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { state, actions, computed } = useCharacterForm(location.state?.draftCharacter);

  const [audioProviders, setAudioProviders] = React.useState<AudioProvider[]>([]);
  const [userVoices, setUserVoices] = React.useState<UserVoice[]>([]);
  const [providerVoices, setProviderVoices] = React.useState<Record<string, CachedVoice[]>>({});
  const [loadingVoices, setLoadingVoices] = React.useState(false);
  const [voiceError, setVoiceError] = React.useState<string | null>(null);
  const [hasLoadedVoices, setHasLoadedVoices] = React.useState(false);

  const loadVoices = React.useCallback(async () => {
    setLoadingVoices(true);
    setVoiceError(null);
    try {
      const [providers, voices] = await Promise.all([listAudioProviders(), listUserVoices()]);
      setAudioProviders(providers);
      setUserVoices(voices);

      const voicesByProvider: Record<string, CachedVoice[]> = {};
      await Promise.all(
        providers.map(async (provider) => {
          try {
            if (provider.providerType === "elevenlabs" && provider.apiKey) {
              voicesByProvider[provider.id] = await refreshProviderVoices(provider.id);
            } else {
              voicesByProvider[provider.id] = await getProviderVoices(provider.id);
            }
          } catch (err) {
            console.warn("Failed to refresh provider voices:", err);
            try {
              voicesByProvider[provider.id] = await getProviderVoices(provider.id);
            } catch (fallbackErr) {
              console.warn("Failed to load cached voices:", fallbackErr);
              voicesByProvider[provider.id] = [];
            }
          }
        }),
      );
      setProviderVoices(voicesByProvider);
      setHasLoadedVoices(true);
    } catch (err) {
      console.error("Failed to load voices:", err);
      setVoiceError("Failed to load voices");
    } finally {
      setLoadingVoices(false);
    }
  }, []);

  React.useEffect(() => {
    if (state.step !== Step.Description || hasLoadedVoices) return;
    void loadVoices();
  }, [state.step, hasLoadedVoices, loadVoices]);

  const handleBack = () => {
    if (state.step === Step.Extras) {
      actions.setStep(Step.StartingScene);
    } else if (state.step === Step.StartingScene) {
      actions.setStep(Step.Description);
    } else if (state.step === Step.Description) {
      actions.setStep(Step.Identity);
    } else {
      navigate(-1);
    }
  };

  const handleSave = async () => {
    const success = await actions.handleSave();
    if (success) {
      navigate("/chat");
    }
  };

  //const stepLabel =
  //  state.step === Step.Identity ? "Identity" :
  //  state.step === Step.StartingScene ? "Starting Scene" :
  //  "Description";

  return (
    <div className="flex min-h-screen flex-col bg-surface text-fg">
      <TopNav currentPath={location.pathname + location.search} onBackOverride={handleBack} />

      {/*<ProgressIndicator
        currentStep={state.step}
        stepLabel={stepLabel}
      />*/}

      <main className="flex flex-1 flex-col overflow-y-auto px-4 pb-20 pt-[calc(72px+env(safe-area-inset-top))] lg:px-8 lg:mx-auto lg:w-full lg:max-w-5xl">
        <AnimatePresence mode="wait">
          {state.step === Step.Identity ? (
            <IdentityStep
              key="identity"
              name={state.name}
              onNameChange={actions.setName}
              avatarPath={state.avatarPath}
              onAvatarChange={actions.setAvatarPath}
              avatarCrop={state.avatarCrop}
              onAvatarCropChange={actions.setAvatarCrop}
              avatarRoundPath={state.avatarRoundPath}
              onAvatarRoundChange={actions.setAvatarRoundPath}
              backgroundImagePath={state.backgroundImagePath}
              onBackgroundImageChange={actions.setBackgroundImagePath}
              onBackgroundImageUpload={actions.handleBackgroundImageUpload}
              disableAvatarGradient={state.disableAvatarGradient}
              onDisableAvatarGradientChange={actions.setDisableAvatarGradient}
              onContinue={() => actions.setStep(Step.Description)}
              canContinue={computed.canContinueIdentity}
              importingAvatar={state.importingAvatar}
              avatarImportError={state.avatarImportError}
              onImport={actions.handleImport}
            />
          ) : state.step === Step.Description ? (
            <DescriptionStep
              key="description"
              definition={state.definition}
              onDefinitionChange={actions.setDefinition}
              description={state.description}
              onDescriptionChange={actions.setDescription}
              models={state.models}
              loadingModels={state.loadingModels}
              selectedModelId={state.selectedModelId}
              onSelectModel={actions.setSelectedModelId}
              selectedFallbackModelId={state.selectedFallbackModelId}
              onSelectFallbackModel={actions.setSelectedFallbackModelId}
              memoryType={state.memoryType}
              dynamicMemoryEnabled={state.dynamicMemoryEnabled}
              onMemoryTypeChange={actions.setMemoryType}
              promptTemplates={state.promptTemplates}
              loadingTemplates={state.loadingTemplates}
              systemPromptTemplateId={state.systemPromptTemplateId}
              onSelectSystemPrompt={actions.setSystemPromptTemplateId}
              voiceConfig={state.voiceConfig}
              onVoiceConfigChange={actions.setVoiceConfig}
              voiceAutoplay={state.voiceAutoplay}
              onVoiceAutoplayChange={actions.setVoiceAutoplay}
              audioProviders={audioProviders}
              userVoices={userVoices}
              providerVoices={providerVoices}
              loadingVoices={loadingVoices}
              voiceError={voiceError}
              onSave={() => actions.setStep(Step.StartingScene)}
              canSave={computed.canSaveDescription}
              saving={false}
              error={state.error}
              submitLabel="Continue to Starting Scenes"
            />
          ) : state.step === Step.StartingScene ? (
            <StartingSceneStep
              key="starting-scene"
              scenes={state.scenes}
              onScenesChange={actions.setScenes}
              defaultSceneId={state.defaultSceneId}
              onDefaultSceneIdChange={actions.setDefaultSceneId}
              onContinue={() => actions.setStep(Step.Extras)}
              canContinue={computed.canContinueStartingScene}
            />
          ) : (
            <ExtrasStep
              key="extras"
              nickname={state.nickname}
              onNicknameChange={actions.setNickname}
              creator={state.creator}
              onCreatorChange={actions.setCreator}
              creatorNotes={state.creatorNotes}
              onCreatorNotesChange={actions.setCreatorNotes}
              creatorNotesMultilingualText={state.creatorNotesMultilingualText}
              onCreatorNotesMultilingualTextChange={actions.setCreatorNotesMultilingualText}
              tagsText={state.tagsText}
              onTagsTextChange={actions.setTagsText}
              onSave={handleSave}
              saving={state.saving}
              error={state.error}
            />
          )}
        </AnimatePresence>
      </main>
    </div>
  );
}
