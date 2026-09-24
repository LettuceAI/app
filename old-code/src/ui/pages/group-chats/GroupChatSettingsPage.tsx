import {
  ArrowLeft,
  User,
  Plus,
  Trash2,
  Edit2,
  Check,
  X,
  Image as ImageIcon,
  ChevronRight,
  Copy,
  GitBranch,
  Brain,
  BarChart3,
  RefreshCw,
  Download,
  Upload,
  Volume2,
  VolumeX,
  BookOpen,
  NotebookPen,
  Clapperboard,
  History,
  Settings,
} from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { BottomMenu, MenuSection } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";
import { useGroupChatSettingsController } from "./hooks/useGroupChatSettingsController";
import { useGroupChatLayoutContext } from "./GroupChatLayout";
import {
  SectionHeader,
  CharacterAvatar,
  QuickChip,
  PersonaSelector,
  GroupCharacterModelsSection,
  GroupPromptTemplateSection,
} from "./components/settings";
import { OptionRow } from "./components/OptionRow";
import { GroupAuthorNoteBottomMenu } from "./components";
import { Switch } from "../../components/Switch";
import { processBackgroundImage } from "../../../core/utils/image";
import { storageBridge } from "../../../core/storage/files";
import {
  hasGroupSessionOverride,
  type GroupSessionOverrideKey,
} from "../../../core/storage/schemas";
import { toast } from "../../components/toast";
import { useAvatar } from "../../hooks/useAvatar";
import { AvatarImage } from "../../components/AvatarImage";
import React, { useState } from "react";
import { useI18n } from "../../../core/i18n/context";

const PARTICIPATION_COLORS = [
  "bg-accent",
  "bg-info",
  "bg-secondary",
  "bg-warning",
  "bg-danger",
  "bg-lime-400",
  "bg-cyan-400",
  "bg-fuchsia-400",
];

function OverrideStatusRow({
  show,
  label,
  onReset,
  disabled = false,
}: {
  show: boolean;
  label?: string;
  onReset: () => void;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  if (!show) return null;
  return (
    <div className="mt-1.5 flex items-center justify-between gap-2 px-1">
      <span className={cn(typography.caption.size, "text-fg/45")}>
        {label ?? t("groupChats.overrides.overriddenLabel")}
      </span>
      <button
        type="button"
        onClick={onReset}
        disabled={disabled}
        className={cn(
          typography.caption.size,
          "font-medium text-accent/80 transition hover:text-accent disabled:opacity-50",
        )}
      >
        {t("groupChats.overrides.useGroupDefault")}
      </button>
    </div>
  );
}

// Main Component
// ============================================================================

export function GroupChatSettingsPage({
  mode = "page",
  onClose,
  groupSessionId: groupSessionIdProp,
}: {
  mode?: "page" | "drawer";
  onClose?: () => void;
  groupSessionId?: string;
} = {}) {
  const { t } = useI18n();
  const params = useParams<{ groupSessionId: string }>();
  const groupSessionId = groupSessionIdProp ?? params.groupSessionId;
  const isDrawer = mode === "drawer";
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();

  const {
    session: layoutSession,
    characters: layoutCharacters,
    personas: layoutPersonas,
    sessionLoading,
    backgroundImageData,
    updateSession,
    group,
  } = useGroupChatLayoutContext();

  const {
    session,
    personas,
    currentPersona,
    groupCharacters,
    availableCharacters,
    currentPersonaDisplay,
    messageCount,
    ui,
    setEditingName,
    setNameDraft,
    setShowPersonaSelector,
    setShowAddCharacter,
    setShowRemoveConfirm,
    handleSaveName,
    handleChangePersona,
    handleClearOverride,
    handleAddCharacter,
    handleRemoveCharacter,
    handleChangeSpeakerSelectionMethod,
    handleChangeCharacterModel,
    handleChangePromptTemplate,
    handleSetCharacterMuted,
    handleUpdateBackgroundImage,
    handleSetDisableCharacterLorebooks,
    mutedCharacterIds,
    getParticipationPercent,
    participationStats,
  } = useGroupChatSettingsController(groupSessionId, {
    layoutSession,
    layoutCharacters,
    layoutPersonas,
    updateSession,
  });
  const [backgroundImagePath, setBackgroundImagePath] = useState(
    session?.backgroundImagePath || "",
  );
  const [savingBackground, setSavingBackground] = useState(false);
  const [showCloneOptions, setShowCloneOptions] = useState(false);
  const [showBranchOptions, setShowBranchOptions] = useState(false);
  const [showChatpkgImportMapMenu, setShowChatpkgImportMapMenu] = useState(false);
  const [pendingChatpkgImport, setPendingChatpkgImport] = useState<{
    path: string;
    info: any;
  } | null>(null);
  const [chatpkgParticipantMap, setChatpkgParticipantMap] = useState<Record<string, string>>({});
  const [importingChatpkg, setImportingChatpkg] = useState(false);
  const [cloning, setCloning] = useState(false);
  const [branching, setBranching] = useState(false);
  const [creatingChat, setCreatingChat] = useState(false);
  const personaAvatarUrl = useAvatar(
    "persona",
    currentPersona?.id ?? "",
    currentPersona?.avatarPath,
    "round",
  );

  // Sync backgroundImagePath with session when it changes
  React.useEffect(() => {
    if (session?.backgroundImagePath !== undefined) {
      setBackgroundImagePath(session.backgroundImagePath || "");
    }
  }, [session?.backgroundImagePath]);

  const handleBackgroundImageUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file || !groupSessionId) return;

    const input = event.target;
    setSavingBackground(true);
    void processBackgroundImage(file)
      .then(async (dataUrl: string) => {
        setBackgroundImagePath(dataUrl);
        await handleUpdateBackgroundImage(dataUrl);
      })
      .catch((error: unknown) => {
        console.warn("Failed to process background image", error);
      })
      .finally(() => {
        input.value = "";
        setSavingBackground(false);
      });
  };

  const handleRemoveBackground = async () => {
    if (!groupSessionId) return;
    setSavingBackground(true);
    try {
      setBackgroundImagePath("");
      await handleUpdateBackgroundImage(null);
    } catch (error) {
      console.error("Failed to remove background:", error);
    } finally {
      setSavingBackground(false);
    }
  };

  const {
    loading,
    error,
    editingName,
    nameDraft,
    showPersonaSelector,
    showAddCharacter,
    showRemoveConfirm,
    saving,
  } = ui;

  const [showAuthorNoteMenu, setShowAuthorNoteMenu] = useState(false);

  const handleBack = () => {
    if (isDrawer) {
      onClose?.();
      return;
    }
    if (groupSessionId) {
      backOrReplace(Routes.groupChat(groupSessionId));
    } else {
      backOrReplace(Routes.groupChats);
    }
  };

  const hasProfile = Boolean(session?.groupCharacterId && group);

  const isOverridden = (key: GroupSessionOverrideKey) =>
    Boolean(session?.groupCharacterId) && !!session && hasGroupSessionOverride(session, key);

  const participantsOverridden = isOverridden("characterIds");
  const mutedOverridden = isOverridden("mutedCharacterIds");
  const participantOverrideLabel =
    participantsOverridden && mutedOverridden
      ? t("groupChats.overrides.participantsAndMutedOverridden")
      : participantsOverridden
        ? t("groupChats.overrides.participantsOverridden")
        : t("groupChats.overrides.mutedOverridden");

  const resetParticipantOverrides = async () => {
    if (participantsOverridden) await handleClearOverride("characterIds");
    if (mutedOverridden) await handleClearOverride("mutedCharacterIds");
  };

  const goToFromSettings = (path: string) => {
    if (isDrawer) onClose?.();
    navigate(path);
  };

  const handleNewChat = async () => {
    if (!session?.groupCharacterId || creatingChat) return;
    setCreatingChat(true);
    try {
      const newSession = await storageBridge.groupCreateSession(session.groupCharacterId);
      goToFromSettings(Routes.groupChat(newSession.id));
    } catch (err) {
      console.error("Failed to create new group chat:", err);
      toast.error(t("groupChats.list.startChatFailed"));
    } finally {
      setCreatingChat(false);
    }
  };

  const handleClone = async (includeMessages: boolean) => {
    if (!session) return;
    try {
      setCloning(true);
      const newSession = await storageBridge.groupSessionDuplicateWithMessages(
        session.id,
        includeMessages,
        `${session.name} (copy)`,
      );
      setShowCloneOptions(false);
      navigate(Routes.groupChat(newSession.id));
    } catch (err) {
      console.error("Failed to clone group:", err);
    } finally {
      setCloning(false);
    }
  };

  const handleBranch = async (characterId: string) => {
    if (!session) return;
    try {
      setBranching(true);
      const newSession = await storageBridge.groupSessionBranchToCharacter(session.id, characterId);
      setShowBranchOptions(false);
      navigate(`/chat/${newSession.characterId}?sessionId=${newSession.id}`);
    } catch (err) {
      console.error("Failed to branch to character:", err);
    } finally {
      setBranching(false);
    }
  };

  const handleExportGroupChatpkg = async () => {
    if (!session) return;
    try {
      const path = await storageBridge.jsonlExportGroupChat(session.id);
      alert(`Group chat exported to:\n${path}`);
    } catch (err) {
      console.error("Failed to export group chat:", err);
      alert(typeof err === "string" ? err : "Failed to export group chat");
    }
  };

  const handleOpenImportGroupChatpkg = async () => {
    try {
      const picked = await storageBridge.jsonlPickFile();
      if (!picked) return;
      const info = await storageBridge.jsonlInspect(picked.path);
      if (info?.type !== "group_chat") {
        alert("This file is not a group chat (JSONL).");
        return;
      }

      const participants = Array.isArray(info?.participants) ? info.participants : [];
      const initialMap: Record<string, string> = {};
      for (const participant of participants) {
        const speakerName = typeof participant?.name === "string" ? participant.name : null;
        if (!speakerName) continue;
        const byName = availableCharacters.find(
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
      alert(typeof err === "string" ? err : "Failed to inspect group chat");
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
        navigate(Routes.groupChat(importedSessionId));
      }
    } catch (err) {
      console.error("Failed to import group chat:", err);
      alert(typeof err === "string" ? err : "Failed to import group chat");
    } finally {
      setImportingChatpkg(false);
    }
  };

  const handleImportGroupChatpkg = async () => {
    if (!pendingChatpkgImport) return;
    await runGroupImport(pendingChatpkgImport.path, chatpkgParticipantMap);
  };

  // Loading state
  if ((sessionLoading || loading) && !session) {
    return (
      <div className="flex h-full flex-col text-fg">
        <header className="shrink-0 border-b border-fg/10 px-4 pb-3 pt-10">
          <div className="flex items-center gap-3">
            <div className="h-8 w-8 animate-pulse rounded-full bg-fg/10" />
            <div className="flex-1 space-y-2">
              <div className="h-5 w-1/3 animate-pulse rounded bg-fg/10" />
              <div className="h-3 w-1/4 animate-pulse rounded bg-fg/10" />
            </div>
          </div>
        </header>
        <main className="flex-1 p-4">
          <div className="space-y-4">
            <div className="h-20 animate-pulse rounded-xl bg-fg/5" />
            <div className="h-20 animate-pulse rounded-xl bg-fg/5" />
            <div className="h-40 animate-pulse rounded-xl bg-fg/5" />
          </div>
        </main>
      </div>
    );
  }

  // Error state
  if (error || !session) {
    return (
      <div className="flex h-full flex-col items-center justify-center text-fg p-8">
        <p className="text-lg font-medium text-danger">{error || t("common.labels.untitled")}</p>
        <button
          onClick={() => navigate(Routes.groupChats)}
          className="mt-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-2 text-sm"
        >
          {t("groupChats.chatSettingsExtra.backToGroupChats")}
        </button>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "relative flex h-full flex-col text-fg",
        !backgroundImagePath && "bg-surface",
      )}
    >
      {/* Background image + scrim overlay */}
      {backgroundImagePath && !isDrawer && (
        <>
          <div
            className="pointer-events-none fixed inset-0 z-0"
            style={{
              backgroundImage: `url(${backgroundImageData || backgroundImagePath})`,
              backgroundSize: "cover",
              backgroundPosition: "center",
              backgroundRepeat: "no-repeat",
            }}
            aria-hidden="true"
          />
          <div
            className="pointer-events-none fixed inset-0 z-0 bg-surface-el/40"
            aria-hidden="true"
          />
        </>
      )}

      {/* Header */}
      {!isDrawer && (
        <header
          className={cn(
            "z-20 shrink-0 border-b border-fg/10 px-4 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] shrink-0",
            !backgroundImagePath ? "bg-surface" : "",
          )}
        >
          <div className="flex items-center gap-3">
            <div className="flex flex-1 items-center min-w-0">
              <button
                onClick={handleBack}
                className="flex shrink-0 px-[0.6em] py-[0.3em] items-center justify-center -ml-2 text-fg transition hover:text-fg/80"
                aria-label={t("groupChats.chatSettingsExtra.backAria")}
              >
                <ArrowLeft size={14} strokeWidth={2.5} />
              </button>
              <div className="min-w-0 flex-1 text-left">
                <p className="truncate text-xl font-bold text-fg/90">{t("common.nav.settings")}</p>
                <p className="mt-0.5 truncate text-xs text-fg/50">
                  {t("groupChats.sessionSettings.subtitle")}
                </p>
              </div>
            </div>
          </div>
        </header>
      )}

      {/* Content */}
      <main className="relative z-10 flex-1 overflow-y-auto px-3 pt-4 pb-16">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
          className={spacing.section}
        >
          {/* Group Header Card - Name + Background  */}
          <section className={spacing.item}>
            <div
              className={cn(
                radius.lg,
                "border border-fg/10 bg-surface-el/85 backdrop-blur-sm overflow-hidden",
              )}
            >
              {/* Background Preview */}
              {backgroundImagePath ? (
                <div className="relative h-28">
                  <img
                    src={backgroundImagePath}
                    alt={t("groupChats.chatSettingsExtra.backgroundAlt")}
                    className="h-full w-full object-cover"
                  />
                  <div className="absolute inset-0 bg-linear-to-t from-surface-el/90 to-transparent" />
                  <div className="absolute right-2 top-2 flex items-center gap-1.5">
                    <label
                      title={t("groupChats.sessionSettings.changeBackground")}
                      className={cn(
                        "flex h-7 w-7 cursor-pointer items-center justify-center",
                        radius.full,
                        "bg-surface-el/60 text-fg/70 backdrop-blur-sm",
                        interactive.transition.fast,
                        "hover:bg-fg/20 hover:text-fg",
                        savingBackground && "pointer-events-none opacity-50",
                      )}
                    >
                      <ImageIcon className="h-3.5 w-3.5" />
                      <input
                        type="file"
                        accept="image/*"
                        onChange={handleBackgroundImageUpload}
                        disabled={savingBackground}
                        className="hidden"
                      />
                    </label>
                    <button
                      onClick={handleRemoveBackground}
                      disabled={savingBackground}
                      className={cn(
                        "flex h-7 w-7 items-center justify-center",
                        radius.full,
                        "bg-surface-el/60 text-fg/70 backdrop-blur-sm",
                        interactive.transition.fast,
                        "hover:bg-danger/80 hover:text-fg",
                        "disabled:opacity-50",
                      )}
                      aria-label={t("groupChats.sessionSettings.removeBackground")}
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              ) : null}

              {/* Group Info */}
              <div className="p-4">
                <div className="flex items-center gap-3">
                  <div className="flex shrink-0 -space-x-2.5">
                    {groupCharacters.slice(0, 4).map((character) => (
                      <div key={character.id} className="rounded-full ring-2 ring-surface-el">
                        <CharacterAvatar character={character} size="sm" />
                      </div>
                    ))}
                    {groupCharacters.length > 4 && (
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-fg/10 text-[10px] font-bold text-fg/60 ring-2 ring-surface-el">
                        +{groupCharacters.length - 4}
                      </div>
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    {editingName ? (
                      <div className="flex items-center gap-3">
                        <input
                          type="text"
                          value={nameDraft}
                          onChange={(e) => setNameDraft(e.target.value)}
                          className={cn(
                            "min-w-0 flex-1 bg-transparent py-1",
                            typography.body.size,
                            typography.body.weight,
                            "text-fg placeholder-fg/30",
                            "border-b border-accent/50 focus:border-accent",
                            "focus:outline-none transition-colors",
                          )}
                          placeholder={t("groupChats.sessionSettings.enterGroupName")}
                          autoFocus
                        />
                        <button
                          onClick={handleSaveName}
                          disabled={saving || !nameDraft.trim()}
                          className={cn(
                            "flex items-center justify-center p-1.5",
                            radius.full,
                            "bg-accent/20 text-accent/80",
                            interactive.transition.default,
                            "hover:bg-accent/30 disabled:opacity-50",
                          )}
                        >
                          <Check size={14} />
                        </button>
                        <button
                          onClick={() => {
                            setNameDraft(session.name);
                            setEditingName(false);
                          }}
                          className={cn(
                            "flex items-center justify-center p-1.5",
                            radius.full,
                            "bg-fg/10 text-fg/60",
                            interactive.transition.default,
                            "hover:bg-fg/20",
                          )}
                        >
                          <X size={14} />
                        </button>
                      </div>
                    ) : (
                      <button
                        onClick={() => setEditingName(true)}
                        className="flex w-full items-center justify-between gap-3 text-left group"
                      >
                        <div className="min-w-0">
                          <p
                            className={cn(
                              typography.h3.size,
                              typography.h3.weight,
                              "text-fg truncate",
                            )}
                          >
                            {session.name}
                          </p>
                          <p className={cn(typography.caption.size, "text-fg/45 mt-0.5")}>
                            {groupCharacters.length}{" "}
                            {groupCharacters.length === 1
                              ? t("groupChats.sessionSettings.participant")
                              : t("groupChats.sessionSettings.participants")}
                            <span className="opacity-50 mx-1.5">•</span>
                            {messageCount}{" "}
                            {messageCount === 1
                              ? t("groupChats.sessionSettings.message")
                              : t("groupChats.sessionSettings.messages")}
                          </p>
                        </div>
                        <Edit2 className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
                      </button>
                    )}
                  </div>
                </div>

                {!backgroundImagePath && (
                  <label
                    className={cn(
                      "flex cursor-pointer items-center gap-2 mt-3 py-2 px-3",
                      radius.md,
                      "border border-dashed border-fg/15 text-fg/50",
                      interactive.transition.default,
                      "hover:border-fg/25 hover:bg-fg/5 hover:text-fg/70",
                      savingBackground && "opacity-50 cursor-not-allowed",
                    )}
                  >
                    <ImageIcon className="h-4 w-4" />
                    <span className={cn(typography.caption.size)}>
                      {savingBackground
                        ? t("groupChats.sessionSettings.uploading")
                        : t("groupChats.sessionSettings.addBackgroundImage")}
                    </span>
                    <input
                      type="file"
                      accept="image/*"
                      onChange={handleBackgroundImageUpload}
                      disabled={savingBackground}
                      className="hidden"
                    />
                  </label>
                )}
                <OverrideStatusRow
                  show={isOverridden("backgroundImagePath")}
                  onReset={() => void handleClearOverride("backgroundImagePath")}
                  disabled={saving || savingBackground}
                />
              </div>
            </div>
          </section>

          {hasProfile && group && (
            <section className={spacing.item}>
              <SectionHeader
                title={t("groupChats.sessionSettings.groupActionsTitle")}
                subtitle={t("groupChats.sessionSettings.groupActionsSubtitle", {
                  name: group.name,
                })}
              />
              <div className={spacing.field}>
                <QuickChip
                  icon={<Plus className="h-4 w-4" />}
                  label={t("groupChats.list.newChat")}
                  value={
                    creatingChat
                      ? t("groupChats.list.startingChat")
                      : t("groupChats.sessionSettings.newChatDesc")
                  }
                  onClick={() => void handleNewChat()}
                  disabled={creatingChat}
                />
                <QuickChip
                  icon={<History className="h-4 w-4" />}
                  label={t("groupChats.list.chatHistory")}
                  value={t("groupChats.sessionSettings.chatHistoryDesc")}
                  onClick={() => goToFromSettings(Routes.groupChatHistory(group.id))}
                />
                <QuickChip
                  icon={<Settings className="h-4 w-4" />}
                  label={t("groupChats.list.groupSettings")}
                  value={t("groupChats.sessionSettings.groupSettingsDesc")}
                  onClick={() => goToFromSettings(Routes.groupSettings(group.id))}
                />
              </div>
              <p className={cn(typography.caption.size, "mt-2 px-1 text-fg/45")}>
                {t("groupChats.sessionSettings.overrideHint")}
              </p>
            </section>
          )}

          {/* Quick Settings */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.quickSettings")}
              subtitle={t("chats.settings.quickSettingsDesc")}
            />
            <div className="grid grid-cols-1 gap-2">
              <div>
                <QuickChip
                  icon={
                    personaAvatarUrl ? (
                      <div className="h-full w-full overflow-hidden rounded-full">
                        <AvatarImage
                          src={personaAvatarUrl}
                          alt={currentPersona?.title ?? "Persona"}
                          crop={currentPersona?.avatarCrop}
                          applyCrop
                        />
                      </div>
                    ) : (
                      <User className="h-4 w-4" />
                    )
                  }
                  label={t("groupChats.sessionSettings.personaLabel")}
                  value={currentPersonaDisplay}
                  onClick={() => setShowPersonaSelector(true)}
                />
                <OverrideStatusRow
                  show={isOverridden("personaId")}
                  onReset={() => void handleClearOverride("personaId")}
                  disabled={saving}
                />
              </div>
              <div>
                <QuickChip
                  icon={<BookOpen className="h-4 w-4" />}
                  label={t("groupChats.settingsPageExtra.manageLorebooks")}
                  value={t("groupChats.sessionSettings.lorebooksAttached", {
                    count: String(session.lorebookIds?.length ?? 0),
                  })}
                  onClick={() => navigate(Routes.groupChatLorebook(session.id))}
                />
                <OverrideStatusRow
                  show={isOverridden("lorebookIds")}
                  onReset={() => void handleClearOverride("lorebookIds")}
                  disabled={saving}
                />
              </div>
              <QuickChip
                icon={<NotebookPen className="h-4 w-4" />}
                label={t("chats.authorNote.title")}
                value={
                  session.authorNote?.trim()
                    ? session.authorNote.trim().split("\n")[0]
                    : t("groupChats.sessionSettings.authorNoteEmpty")
                }
                onClick={() => setShowAuthorNoteMenu(true)}
              />
            </div>
            <div>
              <div
                className={cn(
                  "flex w-full items-center justify-between gap-3 rounded-xl border border-fg/10 bg-surface-el/85 px-4 py-3 text-left",
                  interactive.transition.default,
                )}
              >
                <div className="min-w-0">
                  <div className="text-sm font-medium text-fg">
                    {t("groupChats.sessionSettings.disableCharacterLorebooks")}
                  </div>
                  <div className="mt-0.5 text-xs text-fg/50">
                    {t("groupChats.sessionSettings.disableCharacterLorebooksDesc")}
                  </div>
                </div>
                <Switch
                  checked={!!session.disableCharacterLorebooks}
                  onChange={(next) => void handleSetDisableCharacterLorebooks(next)}
                />
              </div>
              <OverrideStatusRow
                show={isOverridden("disableCharacterLorebooks")}
                onReset={() => void handleClearOverride("disableCharacterLorebooks")}
                disabled={saving}
              />
            </div>
          </section>

          {/* Speaker Selection Method */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.sessionSettings.speakerSelection")}
              subtitle={t("groupChats.sessionSettings.speakerSubtitle")}
            />
            <div className="space-y-2">
              <OptionRow
                selected={session.speakerSelectionMethod === "llm"}
                onSelect={() => handleChangeSpeakerSelectionMethod("llm")}
                icon={Brain}
                label={t("groupChats.sessionSettings.llm")}
                description={t("groupChats.sessionSettings.llmDesc")}
                disabled={saving}
              />
              <OptionRow
                selected={session.speakerSelectionMethod === "heuristic"}
                onSelect={() => handleChangeSpeakerSelectionMethod("heuristic")}
                icon={BarChart3}
                label={t("groupChats.sessionSettings.heuristic")}
                description={t("groupChats.sessionSettings.heuristicDesc")}
                disabled={saving}
              />
              <OptionRow
                selected={session.speakerSelectionMethod === "round_robin"}
                onSelect={() => handleChangeSpeakerSelectionMethod("round_robin")}
                icon={RefreshCw}
                label={t("groupChats.sessionSettings.roundRobin")}
                description={t("groupChats.sessionSettings.roundRobinDesc")}
                disabled={saving}
              />
              <OptionRow
                selected={
                  session.speakerSelectionMethod === "director" ||
                  session.speakerSelectionMethod === "director_action"
                }
                onSelect={() => {
                  if (
                    session.speakerSelectionMethod !== "director" &&
                    session.speakerSelectionMethod !== "director_action"
                  ) {
                    handleChangeSpeakerSelectionMethod("director");
                  }
                }}
                icon={Clapperboard}
                label={t("groupChats.sessionSettings.director")}
                description={t("groupChats.create.groupSetup.directorRowDesc")}
                disabled={saving}
              />
              {(session.speakerSelectionMethod === "director" ||
                session.speakerSelectionMethod === "director_action") && (
                <div className="grid grid-cols-2 gap-2 pl-4">
                  {(
                    [
                      {
                        value: "director" as const,
                        label: t("groupChats.sessionSettings.directorCue"),
                        desc: t("groupChats.sessionSettings.directorCueShort"),
                      },
                      {
                        value: "director_action" as const,
                        label: t("groupChats.sessionSettings.directorAction"),
                        desc: t("groupChats.sessionSettings.directorActionShort"),
                      },
                    ] as const
                  ).map((option) => (
                    <button
                      key={option.value}
                      onClick={() => handleChangeSpeakerSelectionMethod(option.value)}
                      disabled={saving}
                      className={cn(
                        "flex flex-col items-center gap-0.5 px-3 py-2",
                        radius.lg,
                        "border text-center",
                        interactive.transition.fast,
                        session.speakerSelectionMethod === option.value
                          ? "border-accent/40 bg-accent/10"
                          : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
                        saving && "opacity-50",
                      )}
                    >
                      <div
                        className={cn(
                          "text-xs font-semibold",
                          session.speakerSelectionMethod === option.value
                            ? "text-accent"
                            : "text-fg/80",
                        )}
                      >
                        {option.label}
                      </div>
                      <div className="text-[10px] text-fg/40">{option.desc}</div>
                    </button>
                  ))}
                </div>
              )}
            </div>
            <OverrideStatusRow
              show={isOverridden("speakerSelectionMethod")}
              onReset={() => void handleClearOverride("speakerSelectionMethod")}
              disabled={saving}
            />
          </section>

          {/* Characters Section */}
          <section className={spacing.item}>
            <div className="flex items-center justify-between mb-3">
              <SectionHeader
                title={t("groupChats.sessionSettings.characters")}
                subtitle={t("groupChats.sessionSettings.participantsActive", {
                  total: String(groupCharacters.length),
                  active: String(
                    groupCharacters.length - (session?.mutedCharacterIds?.length ?? 0),
                  ),
                })}
              />
              <button
                onClick={() => setShowAddCharacter(true)}
                disabled={availableCharacters.length === 0}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5",
                  "rounded-full text-xs font-medium",
                  "border transition",
                  availableCharacters.length === 0
                    ? "border-fg/5 bg-fg/5 text-fg/30 cursor-not-allowed"
                    : "border-accent/30 bg-accent/10 text-accent/80 hover:bg-accent/20",
                )}
              >
                <Plus size={14} />
                {t("groupChats.sessionSettings.add")}
              </button>
            </div>

            {participationStats.length > 0 && (
              <div className="mb-2 flex h-2.5 overflow-hidden rounded-full bg-fg/5">
                {groupCharacters.map((char, index) => {
                  const percent = getParticipationPercent(char.id);
                  return (
                    <div
                      key={char.id}
                      className={cn(
                        PARTICIPATION_COLORS[index % PARTICIPATION_COLORS.length],
                        "transition-all duration-300",
                      )}
                      style={{ width: `${percent}%` }}
                      title={`${char.name}: ${percent}%`}
                    />
                  );
                })}
              </div>
            )}

            <div className="space-y-2">
              <AnimatePresence mode="popLayout">
                {groupCharacters.map((character, index) => {
                  const percent = getParticipationPercent(character.id);
                  const isMuted = mutedCharacterIds.has(character.id);

                  return (
                    <motion.div
                      key={character.id}
                      layout
                      initial={{ opacity: 0, scale: 0.95 }}
                      animate={{ opacity: 1, scale: 1 }}
                      exit={{ opacity: 0, scale: 0.95 }}
                      className={cn(
                        "flex items-center gap-3 p-3",
                        radius.lg,
                        "border border-fg/10 bg-surface-el/85",
                      )}
                    >
                      <div className={cn(isMuted && "opacity-40 grayscale")}>
                        <CharacterAvatar character={character} size="md" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <p
                          className={cn(
                            "text-sm font-medium truncate",
                            isMuted ? "text-fg/50" : "text-fg",
                          )}
                        >
                          {character.name}
                          {isMuted && (
                            <span className="ml-2 text-[10px] text-fg/40">
                              {t("groupChats.sessionSettings.muted")}
                            </span>
                          )}
                        </p>
                      </div>
                      {participationStats.length > 0 && (
                        <span className="flex shrink-0 items-center gap-1.5">
                          <span
                            className={cn(
                              "h-2 w-2 rounded-full",
                              PARTICIPATION_COLORS[index % PARTICIPATION_COLORS.length],
                            )}
                          />
                          <span className="text-xs text-fg/50 tabular-nums">{percent}%</span>
                        </span>
                      )}
                      <button
                        onClick={() => handleSetCharacterMuted(character.id, !isMuted)}
                        className={cn(
                          "flex items-center justify-center rounded-lg p-1.5 transition",
                          isMuted
                            ? "text-amber-300 hover:bg-amber-500/10"
                            : "text-fg/40 hover:text-fg hover:bg-fg/10",
                        )}
                        title={
                          isMuted
                            ? t("groupChats.sessionSettings.unmuteCharacter")
                            : t("groupChats.sessionSettings.muteCharacter")
                        }
                      >
                        {isMuted ? <VolumeX size={14} /> : <Volume2 size={14} />}
                      </button>
                      <button
                        onClick={() => setShowRemoveConfirm(character.id)}
                        disabled={groupCharacters.length <= 2}
                        className={cn(
                          "flex items-center justify-center rounded-lg transition",
                          groupCharacters.length <= 2
                            ? "text-fg/20 cursor-not-allowed"
                            : "text-fg/40 hover:text-danger hover:bg-danger/10",
                        )}
                        title={
                          groupCharacters.length <= 2
                            ? t("groupChats.sessionSettings.minTwoRequired")
                            : t("groupChats.sessionSettings.removeCharacter")
                        }
                      >
                        <Trash2 size={14} />
                      </button>
                    </motion.div>
                  );
                })}
              </AnimatePresence>
            </div>

            {(participantsOverridden || mutedOverridden) && (
              <div className="mt-3 border-t border-fg/8 pt-1.5">
                <OverrideStatusRow
                  show
                  label={participantOverrideLabel}
                  onReset={() => void resetParticipantOverrides()}
                  disabled={saving}
                />
              </div>
            )}
            <div className="mt-4 space-y-1 px-1">
              {groupCharacters.length <= 2 && (
                <p className={cn(typography.caption.size, "leading-snug text-fg/35")}>
                  {t("groupChats.sessionSettings.groupMinCharacters")}
                </p>
              )}
              <p className={cn(typography.caption.size, "leading-snug text-fg/35")}>
                {t("groupChats.sessionSettings.mutedCharactersNote")}
              </p>
            </div>
          </section>

          <section className={spacing.item}>
            <GroupCharacterModelsSection
              characters={groupCharacters}
              overrides={session.characterModelOverrides ?? {}}
              onChange={(characterId, modelId) =>
                void handleChangeCharacterModel(characterId, modelId)
              }
              disabled={saving}
              footer={
                <OverrideStatusRow
                  show={isOverridden("characterModelOverrides")}
                  onReset={() => void handleClearOverride("characterModelOverrides")}
                  disabled={saving}
                />
              }
            />
          </section>

          <section className={spacing.item}>
            <GroupPromptTemplateSection
              chatType={session.chatType}
              templateId={
                session.chatType === "roleplay"
                  ? (session.groupChatRoleplayPromptTemplateId ?? null)
                  : (session.groupChatPromptTemplateId ?? null)
              }
              onChange={(templateId) => void handleChangePromptTemplate(templateId)}
              disabled={saving}
              footer={
                <OverrideStatusRow
                  show={isOverridden(
                    session.chatType === "roleplay"
                      ? "groupChatRoleplayPromptTemplateId"
                      : "groupChatPromptTemplateId",
                  )}
                  onReset={() =>
                    void handleClearOverride(
                      session.chatType === "roleplay"
                        ? "groupChatRoleplayPromptTemplateId"
                        : "groupChatPromptTemplateId",
                    )
                  }
                  disabled={saving}
                />
              }
            />
          </section>

          {/* Session Actions */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.session")}
              subtitle={t("groupChats.sessionSettings.sessionActionsSubtitle")}
            />
            <div className={spacing.field}>
              <QuickChip
                icon={<Download className="h-4 w-4" />}
                label={t("groupChats.sessionSettings.export")}
                value={t("groupChats.sessionSettings.exportDesc")}
                onClick={() => void handleExportGroupChatpkg()}
              />
              <QuickChip
                icon={<Upload className="h-4 w-4" />}
                label={t("groupChats.sessionSettings.import")}
                value={t("groupChats.sessionSettings.importDesc")}
                onClick={() => {
                  void handleOpenImportGroupChatpkg();
                }}
                disabled={importingChatpkg}
              />
              <QuickChip
                icon={<Copy className="h-4 w-4" />}
                label={t("groupChats.sessionSettings.duplicate")}
                value={t("groupChats.sessionSettings.duplicateDesc")}
                onClick={() => setShowCloneOptions(true)}
              />
              <QuickChip
                icon={<GitBranch className="h-4 w-4" />}
                label={t("groupChats.sessionSettings.branchTo1on1")}
                value={t("groupChats.sessionSettings.branchTo1on1Desc")}
                onClick={() => setShowBranchOptions(true)}
              />
            </div>
          </section>
        </motion.div>
      </main>

      {/* Persona Selector Modal */}
      <PersonaSelector
        isOpen={showPersonaSelector}
        onClose={() => setShowPersonaSelector(false)}
        personas={personas}
        selectedPersonaId={session.personaId}
        onSelect={handleChangePersona}
      />

      {/* Author Note Menu */}
      <GroupAuthorNoteBottomMenu
        isOpen={showAuthorNoteMenu}
        onClose={() => setShowAuthorNoteMenu(false)}
        session={session}
        onSaved={(updated) => updateSession(updated)}
      />

      {/* Add Character Modal */}
      <BottomMenu
        isOpen={showAddCharacter}
        onClose={() => setShowAddCharacter(false)}
        title={t("groupChats.sessionSettings.addCharacterTitle")}
      >
        <div className="space-y-2 max-h-[60vh] overflow-y-auto">
          {availableCharacters.length === 0 ? (
            <div className="text-center py-8 text-fg/50 text-sm">
              {t("groupChats.sessionSettings.allCharactersInGroup")}
            </div>
          ) : (
            availableCharacters.map((character) => (
              <button
                key={character.id}
                onClick={() => handleAddCharacter(character.id)}
                disabled={saving}
                className={cn(
                  "flex w-full items-center gap-3 p-3 text-left",
                  radius.lg,
                  "border border-fg/10 bg-surface-el/85",
                  interactive.transition.default,
                  "hover:border-fg/20 hover:bg-fg/10",
                  "disabled:opacity-50",
                )}
              >
                <CharacterAvatar character={character} size="md" />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-fg truncate">{character.name}</p>
                  {(character.description || character.definition) && (
                    <p className="text-xs text-fg/50 truncate mt-0.5">
                      {character.description || character.definition}
                    </p>
                  )}
                </div>
                <Plus className="h-4 w-4 text-accent" />
              </button>
            ))
          )}
        </div>
      </BottomMenu>

      {/* Remove Character Confirmation */}
      <BottomMenu
        isOpen={showRemoveConfirm !== null}
        onClose={() => setShowRemoveConfirm(null)}
        title={t("groupChats.sessionSettings.removeCharacterTitle")}
      >
        {showRemoveConfirm && (
          <div className="space-y-4">
            <p className="text-sm text-fg/70">
              {t("groupChats.sessionSettings.removeCharacterConfirm")}{" "}
              <span className="font-medium text-fg">
                {groupCharacters.find((c) => c.id === showRemoveConfirm)?.name}
              </span>{" "}
              {t("groupChats.sessionSettings.removeCharacterFrom")}
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setShowRemoveConfirm(null)}
                disabled={saving}
                className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
              >
                {t("common.buttons.cancel")}
              </button>
              <button
                onClick={() => handleRemoveCharacter(showRemoveConfirm)}
                disabled={saving}
                className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger transition hover:bg-danger/30 disabled:opacity-50"
              >
                {saving
                  ? t("groupChats.sessionSettings.removing")
                  : t("groupChats.sessionSettings.remove")}
              </button>
            </div>
          </div>
        )}
      </BottomMenu>

      {/* Clone Options Modal */}
      <BottomMenu
        isOpen={showCloneOptions}
        onClose={() => setShowCloneOptions(false)}
        title={t("groupChats.sessionSettings.cloneGroupTitle")}
      >
        <MenuSection>
          <div className={spacing.field}>
            <button
              onClick={() => handleClone(true)}
              disabled={cloning}
              className={cn(
                "group flex w-full items-center justify-between p-4",
                radius.md,
                "border text-left",
                interactive.transition.default,
                interactive.active.scale,
                "border-fg/10 bg-surface-el/85 hover:border-fg/20 hover:bg-fg/10",
                cloning && "opacity-50 cursor-not-allowed",
              )}
            >
              <div className="flex items-center gap-3 min-w-0">
                <div
                  className={cn(
                    "flex h-10 w-10 items-center justify-center",
                    radius.full,
                    "border border-accent/30 bg-accent/10 text-accent/80",
                  )}
                >
                  <Copy className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className={cn(typography.body.size, typography.body.weight, "text-fg")}>
                    {t("groupChats.sessionSettings.withMessages")}
                  </p>
                  <p className={cn(typography.caption.size, "text-fg/50 mt-0.5")}>
                    {t("groupChats.sessionSettings.withMessagesDesc")}
                  </p>
                </div>
              </div>
            </button>

            <button
              onClick={() => handleClone(false)}
              disabled={cloning}
              className={cn(
                "group flex w-full items-center justify-between p-4",
                radius.md,
                "border text-left",
                interactive.transition.default,
                interactive.active.scale,
                "border-fg/10 bg-surface-el/85 hover:border-fg/20 hover:bg-fg/10",
                cloning && "opacity-50 cursor-not-allowed",
              )}
            >
              <div className="flex items-center gap-3 min-w-0">
                <div
                  className={cn(
                    "flex h-10 w-10 items-center justify-center",
                    radius.full,
                    "border border-fg/15 bg-fg/10 text-fg/80",
                  )}
                >
                  <Copy className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className={cn(typography.body.size, typography.body.weight, "text-fg")}>
                    {t("groupChats.sessionSettings.withoutMessages")}
                  </p>
                  <p className={cn(typography.caption.size, "text-fg/50 mt-0.5")}>
                    {t("groupChats.sessionSettings.withoutMessagesDesc")}
                  </p>
                </div>
              </div>
            </button>
          </div>
        </MenuSection>
      </BottomMenu>

      {/* Branch to Character Modal */}
      <BottomMenu
        isOpen={showBranchOptions}
        onClose={() => setShowBranchOptions(false)}
        title={t("groupChats.sessionSettings.branchWithCharacterTitle")}
      >
        <MenuSection>
          <p className={cn(typography.bodySmall.size, "text-fg/60 mb-3 px-1")}>
            {t("groupChats.sessionSettings.branchWithCharacterDesc")}
          </p>
          <div className={spacing.field}>
            {groupCharacters.map((character) => (
              <button
                key={character.id}
                onClick={() => handleBranch(character.id)}
                disabled={branching}
                className={cn(
                  "group flex w-full items-center justify-between p-4",
                  radius.md,
                  "border text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-fg/10 bg-surface-el/85 hover:border-fg/20 hover:bg-fg/10",
                  branching && "opacity-50 cursor-not-allowed",
                )}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <CharacterAvatar character={character} size="sm" />
                  <div className="min-w-0">
                    <p
                      className={cn(
                        typography.body.size,
                        typography.body.weight,
                        "text-fg truncate",
                      )}
                    >
                      {character.name}
                    </p>
                    <p className={cn(typography.caption.size, "text-fg/50 mt-0.5 truncate")}>
                      {t("groupChats.sessionSettings.continueWith", { name: character.name })}
                    </p>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
              </button>
            ))}
          </div>
        </MenuSection>
      </BottomMenu>

<BottomMenu
        isOpen={showChatpkgImportMapMenu}
        onClose={() => {
          if (importingChatpkg) return;
          setShowChatpkgImportMapMenu(false);
          setPendingChatpkgImport(null);
          setChatpkgParticipantMap({});
        }}
        title={t("groupChats.sessionSettings.mapParticipantsTitle")}
      >
        <MenuSection>
          <div className="space-y-3 max-h-[60vh] overflow-y-auto">
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
                  <p className={cn(typography.bodySmall.size, "font-medium text-fg")}>
                    {displayName}
                  </p>
                  <p className={cn(typography.caption.size, "mt-0.5 text-fg/50")}>
                    {t("groupChats.sessionSettings.selectLocalCharacter")}
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
                    <option value="">
                      {t("groupChats.sessionSettings.selectCharacterPlaceholder")}
                    </option>
                    {availableCharacters.map((character) => (
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
            onClick={() => {
              void handleImportGroupChatpkg();
            }}
            disabled={importingChatpkg}
            className="mt-4 w-full rounded-xl border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-200 hover:bg-emerald-500/30 disabled:opacity-50"
          >
            {importingChatpkg
              ? t("groupChats.sessionSettings.importing")
              : t("common.buttons.import")}
          </button>
        </MenuSection>
      </BottomMenu>
    </div>
  );
}
