import { useNavigate, useLocation } from "react-router-dom";
import { AnimatePresence } from "framer-motion";

import { Routes } from "../../navigation";
import { TopNav } from "../../components/App";
import { useGroupChatCreateForm, Step } from "./hooks/useGroupChatCreateForm";
import { CharacterSelectStep } from "./components/create/CharacterSelectStep";
import { GroupSetupStep } from "./components/create/GroupSetupStep";
import { GroupStartingSceneStep } from "./components/create/GroupStartingSceneStep";

export function GroupChatCreatePage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { state, actions, computed } = useGroupChatCreateForm({
    onCreated: (sessionId) => navigate(Routes.groupChat(sessionId), { replace: true }),
  });

  const handleBack = () => {
    if (state.step === Step.StartingScene) {
      actions.setStep(Step.GroupSetup);
    } else if (state.step === Step.GroupSetup) {
      actions.setStep(Step.SelectCharacters);
    } else {
      navigate(Routes.groupChats);
    }
  };

  const handleContinueFromSetup = () => {
    if (state.chatType === "roleplay") {
      actions.setStep(Step.StartingScene);
    } else {
      // For conversation, just create the group
      actions.handleCreate();
    }
  };

  return (
    <div className="flex min-h-screen flex-col bg-[#050505] text-gray-100">
      <TopNav currentPath={location.pathname + location.search} onBackOverride={handleBack} />

      <main className="flex flex-1 flex-col overflow-y-auto px-4 pb-6 pt-[calc(72px+env(safe-area-inset-top))]">
        <AnimatePresence mode="wait">
          {state.step === Step.SelectCharacters ? (
            <CharacterSelectStep
              key="select-characters"
              characters={state.characters}
              selectedIds={state.selectedIds}
              onToggleCharacter={actions.toggleCharacter}
              loading={state.loadingCharacters}
              onContinue={() => actions.setStep(Step.GroupSetup)}
              canContinue={computed.canContinueFromCharacters}
            />
          ) : state.step === Step.GroupSetup ? (
            <GroupSetupStep
              key="group-setup"
              chatType={state.chatType}
              onChatTypeChange={actions.setChatType}
              speakerSelectionMethod={state.speakerSelectionMethod}
              onSpeakerSelectionMethodChange={actions.setSpeakerSelectionMethod}
              groupName={state.groupName}
              onGroupNameChange={actions.setGroupName}
              backgroundImagePath={state.backgroundImagePath}
              onBackgroundImageChange={actions.setBackgroundImagePath}
              namePlaceholder={computed.defaultName || "Enter group name..."}
              onContinue={handleContinueFromSetup}
              canContinue={computed.canContinueFromSetup}
            />
          ) : (
            <GroupStartingSceneStep
              key="starting-scene"
              sceneSource={state.sceneSource}
              onSceneSourceChange={actions.setSceneSource}
              customScene={state.customScene}
              onCustomSceneChange={actions.setCustomScene}
              selectedCharacterSceneId={state.selectedCharacterSceneId}
              onSelectedCharacterSceneIdChange={actions.setSelectedCharacterSceneId}
              availableScenes={computed.availableScenes}
              selectedCharacters={computed.selectedCharacters}
              onCreateGroup={actions.handleCreate}
              canCreate={computed.canCreate}
              creating={state.creating}
              error={state.error}
            />
          )}
        </AnimatePresence>
      </main>
    </div>
  );
}
