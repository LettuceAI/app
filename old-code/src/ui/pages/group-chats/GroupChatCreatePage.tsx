import { useNavigate, useLocation } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
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
import { typography, radius, interactive, shadows, cn } from "../../design-tokens";

export function GroupChatCreatePage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const location = useLocation();
  const [showChatpkgImportMapMenu, setShowChatpkgImportMapMenu] = useState(false);
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
      const picked = await storageBridge.jsonlPickFile();
      if (!picked) return;
      const info = await storageBridge.jsonlInspect(picked.path);
      if (info?.type !== "group_chat") {
        alert(t("groupChats.create.invalidPackage"));
        return;
      }

      const participants = Array.isArray(info?.participants) ? info.participants : [];
      const initialMap: Record<string, string> = {};
      for (const participant of participants) {
        const speakerName = typeof participant?.name === "string" ? participant.name : null;
        if (!speakerName) continue;
        const byName = state.characters.find(
          (c) => c.name.trim().toLowerCase() === speakerName.trim().toLowerCase(),
        );
        if (byName) initialMap[speakerName] = byName.id;
      }

      setPendingChatpkgImport({ path: picked.path, info });
      setChatpkgParticipantMap(initialMap);

      const unresolved = participants.some(
        (p: any) => typeof p?.name === "string" && !initialMap[p.name],
      );
      if (unresolved) {
        setShowChatpkgImportMapMenu(true);
      } else {
        await runGroupImport(picked.path, initialMap);
      }
    } catch (err) {
      console.error("Failed to inspect group chat:", err);
      alert(typeof err === "string" ? err : t("groupChats.create.inspectPackageError"));
    }
  };

  const runGroupImport = async (path: string, map: Record<string, string>) => {
    try {
      setImportingChatpkg(true);
      const result = await storageBridge.jsonlImport(path, { participantCharacterMap: map });
      setPendingChatpkgImport(null);
      setShowChatpkgImportMapMenu(false);
      setChatpkgParticipantMap({});
      const importedSessionId = result?.sessionId;
      if (typeof importedSessionId === "string" && importedSessionId.length > 0) {
        navigate(Routes.groupChat(importedSessionId), { replace: true });
      }
    } catch (err) {
      console.error("Failed to import group chat:", err);
      alert(typeof err === "string" ? err : t("groupChats.create.importPackageError"));
    } finally {
      setImportingChatpkg(false);
    }
  };

  const handleImportGroupChatpkg = async () => {
    if (!pendingChatpkgImport) return;
    await runGroupImport(pendingChatpkgImport.path, chatpkgParticipantMap);
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
      actions.handleCreate();
    }
  };

  const ctaLabel =
    state.step === Step.SelectCharacters
      ? t("common.buttons.continue")
      : state.step === Step.GroupSetup
        ? state.chatType === "roleplay"
          ? t("groupChats.create.groupSetup.continueToScene")
          : state.creating
            ? t("common.buttons.creating")
            : t("groupChats.create.groupSetup.createGroupChat")
        : state.creating
          ? t("common.buttons.creating")
          : t("groupChats.create.groupSetup.createGroupChat");

  const ctaEnabled =
    state.step === Step.SelectCharacters
      ? computed.canContinueFromCharacters
      : state.step === Step.GroupSetup
        ? !state.creating
        : computed.canCreate && !state.creating;

  const ctaHint =
    state.step === Step.SelectCharacters
      ? computed.canContinueFromCharacters
        ? t("groupChats.create.characterSelect.selectedCount", {
            count: String(state.selectedIds.size),
          })
        : t("groupChats.create.characterSelect.minHint")
      : null;

  const handleCta = () => {
    if (!ctaEnabled) return;
    if (state.step === Step.SelectCharacters) {
      actions.setStep(Step.GroupSetup);
    } else if (state.step === Step.GroupSetup) {
      handleContinueFromSetup();
    } else {
      actions.handleCreate();
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col bg-surface text-fg">
      <TopNav currentPath={location.pathname + location.search} onBackOverride={handleBack} />

      <main className="flex min-h-0 flex-1 flex-col overflow-y-auto px-4 pb-6 pt-[calc(72px+env(safe-area-inset-top))]">
        <div className="flex w-full flex-1 flex-col">
          <AnimatePresence mode="wait">
            {state.step === Step.SelectCharacters ? (
              <CharacterSelectStep
                key="select-characters"
                characters={state.characters}
                selectedIds={state.selectedIds}
                onToggleCharacter={actions.toggleCharacter}
                loading={state.loadingCharacters}
                onImport={() => {
                  void handleOpenImportGroupChatpkg();
                }}
              />
            ) : state.step === Step.GroupSetup ? (
              <GroupSetupStep
                key="group-setup"
                selectedCharacters={computed.selectedCharacters}
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
                namePlaceholder={computed.defaultName || t("groupChats.sessionSettings.enterGroupName")}
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
              />
            )}
          </AnimatePresence>
        </div>
      </main>

      <div className="shrink-0 border-t border-fg/10 bg-surface px-4 pb-[calc(env(safe-area-inset-bottom)+12px)] pt-3">
        <div className="w-full">
          {state.error && (
            <div
              className={cn(
                "mb-2 px-4 py-2.5",
                radius.lg,
                "border border-danger/30 bg-danger/10",
                "text-sm text-danger",
              )}
            >
              {state.error}
            </div>
          )}
          {ctaHint && (
            <p className={cn(typography.caption.size, "mb-2 text-center text-fg/40")}>{ctaHint}</p>
          )}
          <motion.button
            disabled={!ctaEnabled}
            onClick={handleCta}
            whileTap={{ scale: ctaEnabled ? 0.97 : 1 }}
            className={cn(
              "w-full py-3.5 text-base font-semibold",
              radius.lg,
              interactive.transition.fast,
              ctaEnabled
                ? cn(
                    "border border-accent/40 bg-accent/20 text-accent",
                    shadows.glow,
                    "active:border-accent/60 active:bg-accent/30",
                  )
                : "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30",
            )}
          >
            {ctaLabel}
          </motion.button>
        </div>
      </div>

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
          <div className="max-h-[60vh] space-y-3 overflow-y-auto">
            {(Array.isArray(pendingChatpkgImport?.info?.participants)
              ? pendingChatpkgImport?.info?.participants
              : []
            ).map((participant: any, idx: number) => {
              const participantKey =
                (typeof participant?.name === "string" && participant.name) || `${idx}`;
              const displayName = participantKey;
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
              void handleImportGroupChatpkg();
            }}
            disabled={importingChatpkg}
            className={cn(
              "mt-4 w-full py-3 text-sm font-medium",
              radius.lg,
              "border border-accent/40 bg-accent/20 text-accent",
              interactive.transition.fast,
              "active:bg-accent/30 disabled:opacity-50",
            )}
          >
            {importingChatpkg ? t("common.buttons.importing") : t("common.buttons.import")}
          </button>
        </MenuSection>
      </BottomMenu>
    </div>
  );
}
