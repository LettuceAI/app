import { useNavigate, useLocation } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import { Upload } from "lucide-react";
import { useState } from "react";

import { useI18n } from "../../../core/i18n/context";
import { Routes } from "../../navigation";
import { BottomMenu, MenuSection } from "../../components";
import { TopNav } from "../../components/App";
import { useGroupChatCreateForm, Step } from "./hooks/useGroupChatCreateForm";
import { CharacterSelectStep } from "./components/create/CharacterSelectStep";
import { GroupSetupStep } from "./components/create/GroupSetupStep";
import { GroupStartingSceneStep } from "./components/create/GroupStartingSceneStep";
import { storageBridge } from "../../../core/storage/files";

export function GroupChatCreatePage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const location = useLocation();
  const [showChatpkgImportMapMenu, setShowChatpkgImportMapMenu] = useState(false);
  const [showChatpkgImportConfirmMenu, setShowChatpkgImportConfirmMenu] = useState(false);
  const [pendingChatpkgImport, setPendingChatpkgImport] = useState<{
    path: string;
    info: any;
  } | null>(null);
  const [chatpkgParticipantMap, setChatpkgParticipantMap] = useState<Record<string, string>>({});
  const [importingChatpkg, setImportingChatpkg] = useState(false);
  const { state, actions, computed } = useGroupChatCreateForm({
    onCreated: (sessionId) => navigate(Routes.groupChat(sessionId), { replace: true }),
  });

  const handleOpenImportGroupChatpkg = async () => {
    try {
      const picked = await storageBridge.chatpkgPickFile();
      if (!picked) return;
      const info = await storageBridge.chatpkgInspect(picked.path);
      if (info?.type !== "group_chat") {
        alert(t("groupChats.create.invalidPackage"));
        return;
      }

      const participants = Array.isArray(info?.participants) ? info.participants : [];
      const initialMap: Record<string, string> = {};
      for (const participant of participants) {
        const participantId =
          (typeof participant?.id === "string" && participant.id) ||
          (typeof participant?.characterId === "string" && participant.characterId) ||
          null;
        if (!participantId) continue;
        const participantCharacterId =
          typeof participant?.characterId === "string" ? participant.characterId : null;
        if (participant?.resolved && participantCharacterId) {
          initialMap[participantId] = participantCharacterId;
          continue;
        }
        const displayName =
          typeof participant?.characterDisplayName === "string"
            ? participant.characterDisplayName
            : "Unknown";
        const byName = state.characters.find(
          (c) => c.name.trim().toLowerCase() === displayName.trim().toLowerCase(),
        );
        if (byName) initialMap[participantId] = byName.id;
      }

      setPendingChatpkgImport({ path: picked.path, info });
      setChatpkgParticipantMap(initialMap);

      const unresolved = participants.some((participant: any) => {
        const participantId =
          (typeof participant?.id === "string" && participant.id) ||
          (typeof participant?.characterId === "string" && participant.characterId) ||
          null;
        if (!participantId) return false;
        return !initialMap[participantId];
      });
      if (unresolved) setShowChatpkgImportMapMenu(true);
      else setShowChatpkgImportConfirmMenu(true);
    } catch (err) {
      console.error("Failed to inspect group chat package:", err);
      alert(typeof err === "string" ? err : t("groupChats.create.inspectPackageError"));
    }
  };

  const handleImportGroupChatpkg = async () => {
    if (!pendingChatpkgImport) return;
    try {
      setImportingChatpkg(true);
      const result = await storageBridge.chatpkgImport(pendingChatpkgImport.path, {
        participantCharacterMap: chatpkgParticipantMap,
      });
      const importedSessionId = result?.sessionId;
      if (typeof importedSessionId === "string" && importedSessionId.length > 0) {
        navigate(Routes.groupChat(importedSessionId), { replace: true });
      }
    } catch (err) {
      console.error("Failed to import group chat package:", err);
      alert(typeof err === "string" ? err : t("groupChats.create.importPackageError"));
    } finally {
      setImportingChatpkg(false);
    }
  };

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
    <div className="flex h-full min-h-0 flex-col bg-surface text-fg">
      <TopNav currentPath={location.pathname + location.search} onBackOverride={handleBack} />

      <main className="flex min-h-0 flex-1 flex-col overflow-y-auto px-4 pb-6 pt-[calc(72px+env(safe-area-inset-top))]">
        {state.step === Step.SelectCharacters ? (
          <div className="mb-3 flex justify-end">
            <button
              type="button"
              onClick={() => {
                void handleOpenImportGroupChatpkg();
              }}
              className="inline-flex items-center gap-2 rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-xs font-medium text-emerald-200 transition hover:bg-emerald-500/20"
            >
              <Upload className="h-3.5 w-3.5" />
              {t("groupChats.create.importChatpkg")}
            </button>
          </div>
        ) : null}

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
              memoryType={state.memoryType}
              onMemoryTypeChange={actions.setMemoryType}
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

      <BottomMenu
        isOpen={showChatpkgImportMapMenu}
        onClose={() => {
          if (importingChatpkg) return;
          setShowChatpkgImportMapMenu(false);
          setPendingChatpkgImport(null);
          setChatpkgParticipantMap({});
        }}
        title={t("groupChats.create.mapParticipantsTitle")}
      >
        <MenuSection>
          <div className="space-y-3 max-h-[60vh] overflow-y-auto">
            {(Array.isArray(pendingChatpkgImport?.info?.participants)
              ? pendingChatpkgImport?.info?.participants
              : []
            ).map((participant: any, idx: number) => {
              const participantKey =
                (typeof participant?.id === "string" && participant.id) ||
                (typeof participant?.characterId === "string" && participant.characterId) ||
                `${idx}`;
              const displayName =
                typeof participant?.characterDisplayName === "string"
                  ? participant.characterDisplayName
                  : typeof participant?.displayName === "string"
                    ? participant.displayName
                    : "Unknown";
              const currentValue = chatpkgParticipantMap[participantKey] || "";
              return (
                <div key={participantKey} className="rounded-xl border border-fg/10 bg-fg/5 p-3">
                  <p className="text-sm font-medium text-fg">{displayName}</p>
                  <p className="mt-0.5 text-xs text-fg/50">
                    {t("groupChats.create.selectLocalCharacter")}
                  </p>
                  <select
                    value={currentValue}
                    onChange={(e) => {
                      const next = e.target.value;
                      setChatpkgParticipantMap((prev) => {
                        if (!next) {
                          const clone = { ...prev };
                          delete clone[participantKey];
                          return clone;
                        }
                        return { ...prev, [participantKey]: next };
                      });
                    }}
                    className="mt-2 w-full rounded-lg border border-fg/10 bg-surface-el/40 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none focus:ring-1 focus:ring-fg/10"
                  >
                    <option value="">{t("groupChats.create.selectCharacterPlaceholder")}</option>
                    {state.characters.map((character) => (
                      <option key={character.id} value={character.id}>
                        {character.name}
                      </option>
                    ))}
                  </select>
                </div>
              );
            })}
          </div>
          <button
            type="button"
            onClick={() => {
              setShowChatpkgImportMapMenu(false);
              setShowChatpkgImportConfirmMenu(true);
            }}
            className="mt-4 w-full rounded-xl border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-200 hover:bg-emerald-500/30"
          >
            {t("common.buttons.continue")}
          </button>
        </MenuSection>
      </BottomMenu>

      <BottomMenu
        isOpen={showChatpkgImportConfirmMenu}
        onClose={() => {
          if (importingChatpkg) return;
          setShowChatpkgImportConfirmMenu(false);
          setPendingChatpkgImport(null);
          setChatpkgParticipantMap({});
        }}
        title={t("groupChats.create.importChatPackageTitle")}
      >
        <MenuSection>
          <div className="space-y-4">
            <div className="rounded-xl border border-fg/10 bg-fg/5 p-3 text-sm text-fg/80">
              {t("groupChats.create.importChatPackageDesc")}
            </div>
            <button
              type="button"
              onClick={() => {
                void handleImportGroupChatpkg();
              }}
              disabled={importingChatpkg}
              className="w-full rounded-xl border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-200 transition hover:bg-emerald-500/30 disabled:opacity-50"
            >
              {importingChatpkg ? t("common.buttons.importing") : t("common.buttons.import")}
            </button>
          </div>
        </MenuSection>
      </BottomMenu>
    </div>
  );
}
