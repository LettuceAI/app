import { useNavigate, useLocation } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import React from "react";
import { useI18n } from "../../../core/i18n/context";

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
import { convertFilePathToDataUrl } from "../../../core/storage/images";
import {
  buildBackgroundLibrarySelectionKey,
  buildCharacterCreateLibraryReturnKey,
  type BackgroundLibrarySelectionPayload,
} from "../../components/AvatarPicker/librarySelection";

const CREATE_CHARACTER_DRAFT_KEY = "create-character-draft";

function loadCreateCharacterDraft(locationState: unknown, returnPath: string) {
  if (
    locationState &&
    typeof locationState === "object" &&
    "draftCharacter" in locationState &&
    (locationState as { draftCharacter?: unknown }).draftCharacter
  ) {
    return (locationState as { draftCharacter: unknown }).draftCharacter;
  }

  if (typeof window === "undefined") {
    return undefined;
  }

  const resumeKey = buildCharacterCreateLibraryReturnKey(returnPath);
  if (sessionStorage.getItem(resumeKey) !== "true") {
    sessionStorage.removeItem(CREATE_CHARACTER_DRAFT_KEY);
    return undefined;
  }

  sessionStorage.removeItem(resumeKey);

  const raw = sessionStorage.getItem(CREATE_CHARACTER_DRAFT_KEY);
  if (!raw) {
    return undefined;
  }

  try {
    return JSON.parse(raw);
  } catch (error) {
    console.error("Failed to parse create character draft:", error);
    sessionStorage.removeItem(CREATE_CHARACTER_DRAFT_KEY);
    return undefined;
  }
}

export function CreateCharacterPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useI18n();
  const returnPath = `${location.pathname}${location.search}`;
  const initialDraft = React.useMemo(
    () => loadCreateCharacterDraft(location.state, returnPath),
    [location.state, returnPath],
  );
  const { state, actions, computed } = useCharacterForm(initialDraft);

  const [audioProviders, setAudioProviders] = React.useState<AudioProvider[]>([]);
  const [userVoices, setUserVoices] = React.useState<UserVoice[]>([]);
  const [providerVoices, setProviderVoices] = React.useState<Record<string, CachedVoice[]>>({});
  const [loadingVoices, setLoadingVoices] = React.useState(false);
  const [voiceError, setVoiceError] = React.useState<string | null>(null);
  const [hasLoadedVoices, setHasLoadedVoices] = React.useState(false);
  const markDraftForLibraryReturn = React.useCallback(() => {
    sessionStorage.setItem(buildCharacterCreateLibraryReturnKey(returnPath), "true");
  }, [returnPath]);

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

  React.useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const draft = {
      step: state.step,
      name: state.name,
      avatarPath: state.avatarPath,
      avatarCrop: state.avatarCrop,
      avatarRoundPath: state.avatarRoundPath,
      backgroundImagePath: state.backgroundImagePath,
      definition: state.definition,
      description: state.description,
      nickname: state.nickname,
      creator: state.creator,
      creatorNotes: state.creatorNotes,
      creatorNotesMultilingual: state.creatorNotesMultilingualText.trim()
        ? (() => {
            try {
              return JSON.parse(state.creatorNotesMultilingualText);
            } catch {
              return undefined;
            }
          })()
        : undefined,
      tags: state.tagsText
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
      characterBook: state.importedCharacterBook,
      defaultModelId: state.selectedModelId,
      fallbackModelId: state.selectedFallbackModelId,
      promptTemplateId: state.systemPromptTemplateId,
      companion:
        state.mode === "companion"
          ? {
              ...(state.companion ?? {}),
              prompting: {
                ...(state.companion?.prompting ?? {}),
                promptTemplateId: state.companionPromptTemplateId,
              },
            }
          : undefined,
      mode: state.mode,
      activeLorebookIds: state.activeLorebookIds,
      memoryType: state.memoryType,
      disableAvatarGradient: state.disableAvatarGradient,
      voiceConfig: state.voiceConfig,
      voiceAutoplay: state.voiceAutoplay,
      scenes: state.scenes,
      defaultSceneId: state.defaultSceneId,
    };

    sessionStorage.setItem(CREATE_CHARACTER_DRAFT_KEY, JSON.stringify(draft));
  }, [
    state.step,
    state.name,
    state.avatarPath,
    state.avatarCrop,
    state.avatarRoundPath,
    state.backgroundImagePath,
    state.definition,
    state.description,
    state.nickname,
    state.creator,
    state.creatorNotes,
    state.creatorNotesMultilingualText,
    state.tagsText,
    state.importedCharacterBook,
    state.selectedModelId,
    state.selectedFallbackModelId,
    state.systemPromptTemplateId,
    state.companionPromptTemplateId,
    state.companion,
    state.mode,
    state.activeLorebookIds,
    state.memoryType,
    state.disableAvatarGradient,
    state.voiceConfig,
    state.voiceAutoplay,
    state.scenes,
    state.defaultSceneId,
  ]);

  React.useEffect(() => {
    return () => {
      if (typeof window === "undefined") {
        return;
      }

      const resumeKey = buildCharacterCreateLibraryReturnKey(returnPath);
      if (sessionStorage.getItem(resumeKey) !== "true") {
        sessionStorage.removeItem(CREATE_CHARACTER_DRAFT_KEY);
      }
    };
  }, [returnPath]);

  React.useEffect(() => {
    if (state.loadingModels || state.loadingTemplates) return;

    const storageKey = buildBackgroundLibrarySelectionKey(returnPath);
    const rawSelection = sessionStorage.getItem(storageKey);
    if (!rawSelection) return;

    sessionStorage.removeItem(storageKey);

    let parsed: BackgroundLibrarySelectionPayload | null = null;
    try {
      parsed = JSON.parse(rawSelection) as BackgroundLibrarySelectionPayload;
    } catch (error) {
      console.error("Failed to parse background library selection:", error);
      return;
    }

    if (!parsed?.filePath) return;

    let cancelled = false;
    void (async () => {
      const dataUrl = await convertFilePathToDataUrl(parsed.filePath);
      if (!dataUrl || cancelled) return;
      actions.setBackgroundImagePath(dataUrl);
    })();

    return () => {
      cancelled = true;
    };
  }, [actions, returnPath, state.loadingModels, state.loadingTemplates]);

  const handleChooseBackgroundFromLibrary = React.useCallback(() => {
    markDraftForLibraryReturn();
    navigate("/library/images/pick", {
      state: {
        returnPath,
        selectionKind: "background",
      },
    });
  }, [markDraftForLibraryReturn, navigate, returnPath]);

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
      sessionStorage.removeItem(CREATE_CHARACTER_DRAFT_KEY);
      sessionStorage.removeItem(buildCharacterCreateLibraryReturnKey(returnPath));
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
              onBeforeChooseAvatarFromLibrary={markDraftForLibraryReturn}
              avatarCrop={state.avatarCrop}
              onAvatarCropChange={actions.setAvatarCrop}
              avatarRoundPath={state.avatarRoundPath}
              onAvatarRoundChange={actions.setAvatarRoundPath}
              backgroundImagePath={state.backgroundImagePath}
              onBackgroundImageChange={actions.setBackgroundImagePath}
              onBackgroundImageUpload={actions.handleBackgroundImageUpload}
              onChooseBackgroundFromLibrary={handleChooseBackgroundFromLibrary}
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
              mode={state.mode}
              onModeChange={actions.setMode}
              companion={state.companion}
              onCompanionChange={actions.setCompanionConfig}
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
              companionPromptTemplateId={state.companionPromptTemplateId}
              onSelectCompanionPrompt={actions.setCompanionPromptTemplateId}
              groupChatPromptTemplateId={state.groupChatPromptTemplateId}
              onSelectGroupChatPrompt={actions.setGroupChatPromptTemplateId}
              groupChatRoleplayPromptTemplateId={state.groupChatRoleplayPromptTemplateId}
              onSelectGroupChatRoleplayPrompt={actions.setGroupChatRoleplayPromptTemplateId}
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
              submitLabel={t("characters.scenes.continueToScenes")}
            />
          ) : state.step === Step.StartingScene ? (
            <StartingSceneStep
              key="starting-scene"
              scenes={state.scenes}
              onScenesChange={actions.setScenes}
              defaultSceneId={state.defaultSceneId}
              onDefaultSceneIdChange={actions.setDefaultSceneId}
              mode={state.mode}
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
              activeLorebookIds={state.activeLorebookIds}
              onActiveLorebookIdsChange={actions.setActiveLorebookIds}
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
