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
  Users,
  Volume2,
  VolumeX,
  BookOpen,
} from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { BottomMenu, MenuSection } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";
import { useGroupChatSettingsController } from "./hooks/useGroupChatSettingsController";
import { useGroupChatLayoutContext } from "./GroupChatLayout";
import { SectionHeader, CharacterAvatar, QuickChip, PersonaSelector } from "./components/settings";
import { Switch } from "../../components/Switch";
import { processBackgroundImage } from "../../../core/utils/image";
import { storageBridge } from "../../../core/storage/files";
import { useAvatar } from "../../hooks/useAvatar";
import { AvatarImage } from "../../components/AvatarImage";
import React, { useState } from "react";
import { useI18n } from "../../../core/i18n/context";

// Main Component
// ============================================================================

export function GroupChatSettingsPage() {
  const { t } = useI18n();
  const { groupSessionId } = useParams<{ groupSessionId: string }>();
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();

  const {
    session: layoutSession,
    characters: layoutCharacters,
    personas: layoutPersonas,
    sessionLoading,
    backgroundImageData,
    updateSession,
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
    handleAddCharacter,
    handleRemoveCharacter,
    handleChangeSpeakerSelectionMethod,
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
  const [showChatpkgExportMenu, setShowChatpkgExportMenu] = useState(false);
  const [showChatpkgImportMapMenu, setShowChatpkgImportMapMenu] = useState(false);
  const [showChatpkgImportConfirmMenu, setShowChatpkgImportConfirmMenu] = useState(false);
  const [pendingChatpkgImport, setPendingChatpkgImport] = useState<{
    path: string;
    info: any;
  } | null>(null);
  const [chatpkgParticipantMap, setChatpkgParticipantMap] = useState<Record<string, string>>({});
  const [importingChatpkg, setImportingChatpkg] = useState(false);
  const [cloning, setCloning] = useState(false);
  const [branching, setBranching] = useState(false);
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

  const handleBack = () => {
    if (groupSessionId) {
      backOrReplace(Routes.groupChat(groupSessionId));
    } else {
      backOrReplace(Routes.groupChats);
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

  const handleExportGroupChatpkg = async (includeSnapshots: boolean) => {
    if (!session) return;
    try {
      const path = await storageBridge.chatpkgExportGroupChat(session.id, includeSnapshots);
      setShowChatpkgExportMenu(false);
      alert(`Group chat package exported to:\n${path}`);
    } catch (err) {
      console.error("Failed to export group chat package:", err);
      alert(typeof err === "string" ? err : "Failed to export group chat package");
    }
  };

  const handleOpenImportGroupChatpkg = async () => {
    try {
      const picked = await storageBridge.chatpkgPickFile();
      if (!picked) return;
      const info = await storageBridge.chatpkgInspect(picked.path);
      if (info?.type !== "group_chat") {
        alert("This package is not a group chat package.");
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
        const byName = availableCharacters.find(
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
      alert(typeof err === "string" ? err : "Failed to inspect group chat package");
    }
  };

  const handleImportGroupChatpkg = async () => {
    if (!pendingChatpkgImport) return;
    try {
      setImportingChatpkg(true);
      const result = await storageBridge.chatpkgImport(pendingChatpkgImport.path, {
        participantCharacterMap: chatpkgParticipantMap,
      });
      setPendingChatpkgImport(null);
      setShowChatpkgImportMapMenu(false);
      setShowChatpkgImportConfirmMenu(false);
      const importedSessionId = result?.sessionId;
      if (typeof importedSessionId === "string" && importedSessionId.length > 0) {
        navigate(Routes.groupChat(importedSessionId));
      }
    } catch (err) {
      console.error("Failed to import group chat package:", err);
      alert(typeof err === "string" ? err : "Failed to import group chat package");
    } finally {
      setImportingChatpkg(false);
    }
  };

  // Loading state
  if (sessionLoading || loading) {
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
          Back to Group Chats
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
      {backgroundImagePath && (
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
              aria-label="Back"
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
                <div className="relative h-24">
                  <img
                    src={backgroundImagePath}
                    alt="Background"
                    className="h-full w-full object-cover"
                  />
                  <div className="absolute inset-0 bg-linear-to-t from-surface-el/90 to-transparent" />
                  <button
                    onClick={handleRemoveBackground}
                    disabled={savingBackground}
                    className={cn(
                      "absolute top-2 right-2 flex h-6 w-6 items-center justify-center",
                      radius.full,
                      "bg-surface-el/60 text-fg/70",
                      interactive.transition.fast,
                      "hover:bg-danger/80 hover:text-fg",
                      "disabled:opacity-50",
                    )}
                    aria-label="Remove background"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              ) : null}

              {/* Group Info */}
              <div className="p-4">
                {editingName ? (
                  <div className="flex items-center gap-3">
                    <input
                      type="text"
                      value={nameDraft}
                      onChange={(e) => setNameDraft(e.target.value)}
                      className={cn(
                        "flex-1 bg-transparent py-1",
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
                        "flex items-center justify-center",
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
                        "flex items-center justify-center",
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
                    className="flex w-full items-center justify-between text-left group"
                  >
                    <div className="min-w-0">
                      <p
                        className={cn(typography.h3.size, typography.h3.weight, "text-fg truncate")}
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

                {/* Background action */}
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
                      : backgroundImagePath
                        ? t("groupChats.sessionSettings.changeBackground")
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
              </div>
            </div>
          </section>

          {/* Quick Actions
          <section className={spacing.item}>
            <SectionHeader title="Quick Actions" />
            <div className={spacing.field}>
              <button
                onClick={() => navigate(Routes.groupChatHistory)}
                className={cn(
                  "group flex w-full min-h-14 items-center justify-between",
                  radius.md,
                  "border p-4 text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-fg/10 bg-surface-el/85 backdrop-blur-sm hover:border-fg/20 hover:bg-fg/10",
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
                    <History className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50",
                      )}
                    >
                      Chat History
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-fg truncate")}>
                      View and manage conversations
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
              </button>
            </div>
          </section>*/}

          {/* Persona Section */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.sessionSettings.persona")}
              subtitle={t("groupChats.sessionSettings.personaSubtitle")}
            />
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
          </section>

          {/* Speaker Selection Method */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.sessionSettings.speakerSelection")}
              subtitle={t("groupChats.sessionSettings.speakerSubtitle")}
            />
            <div className="grid grid-cols-3 gap-2">
              {(
                [
                  {
                    value: "llm" as const,
                    label: t("groupChats.sessionSettings.llm"),
                    desc: t("groupChats.sessionSettings.aiPicks"),
                    icon: Brain,
                  },
                  {
                    value: "heuristic" as const,
                    label: t("groupChats.sessionSettings.heuristic"),
                    desc: t("groupChats.sessionSettings.scoreBased"),
                    icon: BarChart3,
                  },
                  {
                    value: "round_robin" as const,
                    label: t("groupChats.sessionSettings.roundRobin"),
                    desc: t("groupChats.sessionSettings.takeTurns"),
                    icon: RefreshCw,
                  },
                ] as const
              ).map((option) => (
                <button
                  key={option.value}
                  onClick={() => handleChangeSpeakerSelectionMethod(option.value)}
                  disabled={saving}
                  className={cn(
                    "relative flex flex-col items-center gap-1.5 p-3",
                    radius.lg,
                    "border text-center",
                    interactive.transition.fast,
                    session.speakerSelectionMethod === option.value
                      ? "border-accent/40 bg-accent/10"
                      : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
                    saving && "opacity-50",
                  )}
                >
                  <option.icon
                    className={cn(
                      "h-5 w-5",
                      session.speakerSelectionMethod === option.value
                        ? "text-accent/80"
                        : "text-fg/50",
                    )}
                  />
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
            <p className={cn(typography.caption.size, "mt-2 text-fg/40")}>
              {session.speakerSelectionMethod === "llm"
                ? t("groupChats.sessionSettings.llmDesc")
                : session.speakerSelectionMethod === "heuristic"
                  ? t("groupChats.sessionSettings.heuristicDesc")
                  : t("groupChats.sessionSettings.roundRobinDesc")}
            </p>
          </section>

          <section className={spacing.item}>
            <SectionHeader
              title="Lorebooks"
              subtitle="Attach session lorebooks and optionally ignore each character's own lorebooks."
            />
            <QuickChip
              icon={<BookOpen className="h-4 w-4" />}
              label="Manage lorebooks"
              value={`${session.lorebookIds?.length ?? 0} attached`}
              onClick={() => navigate(Routes.groupChatLorebook(session.id))}
            />
            <div
              className={cn(
                "mt-2 flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/85 p-3 text-left",
                interactive.transition.default,
              )}
            >
              <div>
                <div className="text-sm font-medium text-fg">Disable character lorebooks</div>
                <div className="text-xs text-fg/50">
                  Group replies will use only the lorebooks attached to this session.
                </div>
              </div>
              <Switch
                checked={!!session.disableCharacterLorebooks}
                onChange={(next) => void handleSetDisableCharacterLorebooks(next)}
              />
            </div>
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

            <div className="space-y-2">
              <AnimatePresence mode="popLayout">
                {groupCharacters.map((character) => {
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
                      <CharacterAvatar character={character} size="md" />
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-fg truncate">
                          {character.name}
                          {isMuted && (
                            <span className="ml-2 text-[10px] text-fg/40">
                              {t("groupChats.sessionSettings.muted")}
                            </span>
                          )}
                        </p>
                        <div className="flex items-center gap-2 mt-1">
                          <div className="flex-1 h-1.5 rounded-full bg-fg/10 overflow-hidden">
                            <div
                              className="h-full bg-accent/60 rounded-full transition-all duration-300"
                              style={{ width: `${percent}%` }}
                            />
                          </div>
                          <span className="text-[10px] text-fg/50 tabular-nums">{percent}%</span>
                        </div>
                      </div>
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

            {groupCharacters.length <= 2 && (
              <p className="mt-2 text-xs text-fg/40 text-center">
                {t("groupChats.sessionSettings.groupMinCharacters")}
              </p>
            )}
            <p className="mt-2 text-xs text-fg/40 text-center">
              {t("groupChats.sessionSettings.mutedCharactersNote")}
            </p>
          </section>

          {/* Session Management */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.sessionSettings.data")}
              subtitle={t("groupChats.sessionSettings.dataSubtitle")}
            />
            <div className={spacing.field}>
              <button
                onClick={() => setShowChatpkgExportMenu(true)}
                className={cn(
                  "group flex w-full min-h-14 items-center justify-between",
                  radius.md,
                  "border p-4 text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-fg/10 bg-surface-el/85 backdrop-blur-sm hover:border-fg/20 hover:bg-fg/10",
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
                    <Download className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50",
                      )}
                    >
                      {t("groupChats.sessionSettings.export")}
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-fg truncate")}>
                      {t("groupChats.sessionSettings.exportDesc")}
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
              </button>

              <button
                onClick={() => {
                  void handleOpenImportGroupChatpkg();
                }}
                disabled={importingChatpkg}
                className={cn(
                  "group flex w-full min-h-14 items-center justify-between",
                  radius.md,
                  "border p-4 text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-fg/10 bg-surface-el/85 backdrop-blur-sm hover:border-fg/20 hover:bg-fg/10",
                  importingChatpkg && "opacity-50",
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
                    <Upload className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50",
                      )}
                    >
                      {t("groupChats.sessionSettings.import")}
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-fg truncate")}>
                      {t("groupChats.sessionSettings.importDesc")}
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
              </button>
            </div>
          </section>

          {/* Session Management */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.sessionSettings.conversation")}
              subtitle={t("groupChats.sessionSettings.conversationSubtitle")}
            />
            <div className={spacing.field}>
              <button
                onClick={() => setShowCloneOptions(true)}
                className={cn(
                  "group flex w-full min-h-14 items-center justify-between",
                  radius.md,
                  "border p-4 text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-fg/10 bg-surface-el/85 backdrop-blur-sm hover:border-fg/20 hover:bg-fg/10",
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
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50",
                      )}
                    >
                      {t("groupChats.sessionSettings.duplicate")}
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-fg truncate")}>
                      {t("groupChats.sessionSettings.duplicateDesc")}
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
              </button>

              <button
                onClick={() => setShowBranchOptions(true)}
                className={cn(
                  "group flex w-full min-h-14 items-center justify-between",
                  radius.md,
                  "border p-4 text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-fg/10 bg-surface-el/85 backdrop-blur-sm hover:border-fg/20 hover:bg-fg/10",
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
                    <GitBranch className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50",
                      )}
                    >
                      {t("groupChats.sessionSettings.branchTo1on1")}
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-fg truncate")}>
                      {t("groupChats.sessionSettings.branchTo1on1Desc")}
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition-colors group-hover:text-fg/60" />
              </button>
            </div>
          </section>

          {/* Participation Stats */}
          {participationStats.length > 0 && (
            <section className={spacing.item}>
              <SectionHeader
                title={t("groupChats.sessionSettings.participation")}
                subtitle={t("groupChats.sessionSettings.participationSubtitle")}
              />
              <div className={cn(radius.lg, "border border-fg/10 bg-surface-el/85 p-4")}>
                {/* Visual bar */}
                <div className="h-3 rounded-full overflow-hidden flex bg-fg/5 mb-4">
                  {groupCharacters.map((char, index) => {
                    const percent = getParticipationPercent(char.id);
                    const colors = [
                      "bg-accent",
                      "bg-info",
                      "bg-secondary",
                      "bg-warning",
                      "bg-danger",
                      "bg-info",
                      "bg-warning",
                      "bg-lime-400",
                    ];
                    return (
                      <div
                        key={char.id}
                        className={cn(colors[index % colors.length])}
                        style={{ width: `${percent}%` }}
                        title={`${char.name}: ${percent}%`}
                      />
                    );
                  })}
                </div>

                {/* Legend */}
                <div className="flex flex-wrap gap-3">
                  {groupCharacters.map((char, index) => {
                    const percent = getParticipationPercent(char.id);
                    const colorDots = [
                      "bg-accent",
                      "bg-info",
                      "bg-secondary",
                      "bg-warning",
                      "bg-danger",
                      "bg-info",
                      "bg-warning",
                      "bg-lime-400",
                    ];
                    return (
                      <div key={char.id} className="flex items-center gap-1.5">
                        <div
                          className={cn(
                            "h-2 w-2 rounded-full",
                            colorDots[index % colorDots.length],
                          )}
                        />
                        <span className="text-xs text-fg/70">{char.name}</span>
                        <span className="text-xs text-fg/40 tabular-nums">({percent}%)</span>
                      </div>
                    );
                  })}
                </div>
              </div>
            </section>
          )}
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
        isOpen={showChatpkgExportMenu}
        onClose={() => setShowChatpkgExportMenu(false)}
        title={t("groupChats.sessionSettings.exportChatPackageTitle")}
      >
        <MenuSection>
          <div className={spacing.field}>
            <button
              onClick={() => {
                void handleExportGroupChatpkg(true);
              }}
              className={cn(
                "group flex w-full items-center justify-between p-4",
                radius.md,
                "border text-left",
                interactive.transition.default,
                interactive.active.scale,
                "border-fg/10 bg-surface-el/85 hover:border-fg/20 hover:bg-fg/10",
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
                  <Users className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className={cn(typography.body.size, typography.body.weight, "text-fg")}>
                    {t("groupChats.sessionSettings.includeCharacterSnapshots")}
                  </p>
                  <p className={cn(typography.caption.size, "text-fg/50 mt-0.5")}>
                    {t("groupChats.sessionSettings.includeCharacterSnapshotsDesc")}
                  </p>
                </div>
              </div>
            </button>

            <button
              onClick={() => {
                void handleExportGroupChatpkg(false);
              }}
              className={cn(
                "group flex w-full items-center justify-between p-4",
                radius.md,
                "border text-left",
                interactive.transition.default,
                interactive.active.scale,
                "border-fg/10 bg-surface-el/85 hover:border-fg/20 hover:bg-fg/10",
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
                  <Download className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className={cn(typography.body.size, typography.body.weight, "text-fg")}>
                    {t("groupChats.sessionSettings.sessionOnly")}
                  </p>
                  <p className={cn(typography.caption.size, "text-fg/50 mt-0.5")}>
                    {t("groupChats.sessionSettings.sessionOnlyDesc")}
                  </p>
                </div>
              </div>
            </button>
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
              setShowChatpkgImportMapMenu(false);
              setShowChatpkgImportConfirmMenu(true);
            }}
            className="mt-4 w-full rounded-xl border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-200 hover:bg-emerald-500/30"
          >
            {t("groupChats.sessionSettings.continue")}
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
        title={t("groupChats.sessionSettings.importChatPackageTitle")}
      >
        <MenuSection>
          <div className="space-y-4">
            <div className="rounded-xl border border-fg/10 bg-fg/5 p-3 text-sm text-fg/80">
              {t("groupChats.sessionSettings.importChatPackageDesc")}
            </div>
            <button
              onClick={() => {
                void handleImportGroupChatpkg();
              }}
              disabled={importingChatpkg}
              className="w-full rounded-xl border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-200 transition hover:bg-emerald-500/30 disabled:opacity-50"
            >
              {importingChatpkg
                ? t("groupChats.sessionSettings.importing")
                : t("common.buttons.import")}
            </button>
          </div>
        </MenuSection>
      </BottomMenu>
    </div>
  );
}
