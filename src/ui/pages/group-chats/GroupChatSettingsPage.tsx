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
} from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { BottomMenu, MenuSection } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";
import { useGroupChatSettingsController } from "./hooks/useGroupChatSettingsController";
import { useGroupChatLayoutContext } from "./GroupChatLayout";
import { SectionHeader, CharacterAvatar, QuickChip, PersonaSelector } from "./components/settings";
import { processBackgroundImage } from "../../../core/utils/image";
import { storageBridge } from "../../../core/storage/files";
import { useAvatar } from "../../hooks/useAvatar";
import { AvatarImage } from "../../components/AvatarImage";
import React, { useState } from "react";

// Main Component
// ============================================================================

export function GroupChatSettingsPage() {
  const { groupSessionId } = useParams<{ groupSessionId: string }>();
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();

  const {
    session: layoutSession,
    characters: layoutCharacters,
    personas: layoutPersonas,
    sessionLoading,
    backgroundImageData,
    reloadSession,
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
    getParticipationPercent,
    participationStats,
  } = useGroupChatSettingsController(groupSessionId, {
    layoutSession,
    layoutCharacters,
    layoutPersonas,
  });
  const [backgroundImagePath, setBackgroundImagePath] = useState(
    session?.backgroundImagePath || "",
  );
  const [savingBackground, setSavingBackground] = useState(false);
  const [showCloneOptions, setShowCloneOptions] = useState(false);
  const [showBranchOptions, setShowBranchOptions] = useState(false);
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
        await storageBridge.groupSessionUpdateBackgroundImage(groupSessionId, dataUrl);
        reloadSession();
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
      await storageBridge.groupSessionUpdateBackgroundImage(groupSessionId, null);
      reloadSession();
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

  // Loading state
  if (sessionLoading || loading) {
    return (
      <div className="flex h-full flex-col text-white">
        <header className="shrink-0 border-b border-white/10 px-4 pb-3 pt-10">
          <div className="flex items-center gap-3">
            <div className="h-8 w-8 animate-pulse rounded-full bg-white/10" />
            <div className="flex-1 space-y-2">
              <div className="h-5 w-1/3 animate-pulse rounded bg-white/10" />
              <div className="h-3 w-1/4 animate-pulse rounded bg-white/10" />
            </div>
          </div>
        </header>
        <main className="flex-1 p-4">
          <div className="space-y-4">
            <div className="h-20 animate-pulse rounded-xl bg-white/5" />
            <div className="h-20 animate-pulse rounded-xl bg-white/5" />
            <div className="h-40 animate-pulse rounded-xl bg-white/5" />
          </div>
        </main>
      </div>
    );
  }

  // Error state
  if (error || !session) {
    return (
      <div className="flex h-full flex-col items-center justify-center text-white p-8">
        <p className="text-lg font-medium text-red-400">{error || "Not found"}</p>
        <button
          onClick={() => navigate(Routes.groupChats)}
          className="mt-4 rounded-xl border border-white/10 bg-white/5 px-4 py-2 text-sm"
        >
          Back to Group Chats
        </button>
      </div>
    );
  }

  return (
    <div
      className="relative flex h-full flex-col text-white overflow-hidden"
      style={{ backgroundColor: backgroundImagePath ? undefined : "#050505" }}
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
          <div className="pointer-events-none fixed inset-0 z-0 bg-black/40" aria-hidden="true" />
        </>
      )}

      {/* Header */}
      <header
        className={cn(
          "relative z-20 shrink-0 border-b border-white/10 px-4 pb-3 pt-10",
          !backgroundImagePath ? "bg-[#050505]" : "",
        )}
      >
        <div className="flex items-center gap-3">
          <button
            onClick={handleBack}
            className="flex shrink-0 px-[0.6em] py-[0.3em] items-center justify-center -ml-2 text-white transition hover:text-white/80"
            aria-label="Back"
          >
            <ArrowLeft size={14} strokeWidth={2.5} />
          </button>
          <div className="min-w-0 flex-1 text-left">
            <p className="truncate text-xl font-bold text-white/90">Group Settings</p>
            <p className="mt-0.5 truncate text-xs text-white/50">Manage group chat preferences</p>
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
                "border border-white/10 bg-[#0c0d13]/85 backdrop-blur-sm overflow-hidden",
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
                  <div className="absolute inset-0 bg-linear-to-t from-[#0c0d13] to-transparent" />
                  <button
                    onClick={handleRemoveBackground}
                    disabled={savingBackground}
                    className={cn(
                      "absolute top-2 right-2 flex h-6 w-6 items-center justify-center",
                      radius.full,
                      "bg-black/60 text-white/70",
                      interactive.transition.fast,
                      "hover:bg-red-500/80 hover:text-white",
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
                        "text-white placeholder-white/30",
                        "border-b border-emerald-400/50 focus:border-emerald-400",
                        "focus:outline-none transition-colors",
                      )}
                      placeholder="Enter group name"
                      autoFocus
                    />
                    <button
                      onClick={handleSaveName}
                      disabled={saving || !nameDraft.trim()}
                      className={cn(
                        "flex items-center justify-center",
                        radius.full,
                        "bg-emerald-400/20 text-emerald-300",
                        interactive.transition.default,
                        "hover:bg-emerald-400/30 disabled:opacity-50",
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
                        "bg-white/10 text-white/60",
                        interactive.transition.default,
                        "hover:bg-white/20",
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
                        className={cn(
                          typography.h3.size,
                          typography.h3.weight,
                          "text-white truncate",
                        )}
                      >
                        {session.name}
                      </p>
                      <p className={cn(typography.caption.size, "text-white/45 mt-0.5")}>
                        {groupCharacters.length}{" "}
                        {groupCharacters.length === 1 ? "participant" : "participants"}
                        <span className="opacity-50 mx-1.5">•</span>
                        {messageCount} {messageCount === 1 ? "message" : "messages"}
                      </p>
                    </div>
                    <Edit2 className="h-4 w-4 shrink-0 text-white/30 transition-colors group-hover:text-white/60" />
                  </button>
                )}

                {/* Background action */}
                <label
                  className={cn(
                    "flex cursor-pointer items-center gap-2 mt-3 py-2 px-3",
                    radius.md,
                    "border border-dashed border-white/15 text-white/50",
                    interactive.transition.default,
                    "hover:border-white/25 hover:bg-white/5 hover:text-white/70",
                    savingBackground && "opacity-50 cursor-not-allowed",
                  )}
                >
                  <ImageIcon className="h-4 w-4" />
                  <span className={cn(typography.caption.size)}>
                    {savingBackground
                      ? "Uploading..."
                      : backgroundImagePath
                        ? "Change background"
                        : "Add background image"}
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
                  "border-white/10 bg-[#0c0d13]/85 backdrop-blur-sm hover:border-white/20 hover:bg-white/10",
                )}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={cn(
                      "flex h-10 w-10 items-center justify-center",
                      radius.full,
                      "border border-white/15 bg-white/10 text-white/80",
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
                        "text-white/50",
                      )}
                    >
                      Chat History
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-white truncate")}>
                      View and manage conversations
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-white/30 transition-colors group-hover:text-white/60" />
              </button>
            </div>
          </section>*/}

          {/* Persona Section */}
          <section className={spacing.item}>
            <SectionHeader title="Persona" subtitle="Your identity in this conversation" />
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
              label="Persona"
              value={currentPersonaDisplay}
              onClick={() => setShowPersonaSelector(true)}
            />
          </section>

          {/* Characters Section */}
          <section className={spacing.item}>
            <div className="flex items-center justify-between mb-3">
              <SectionHeader
                title="Characters"
                subtitle={`${groupCharacters.length} participants`}
              />
              <button
                onClick={() => setShowAddCharacter(true)}
                disabled={availableCharacters.length === 0}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5",
                  "rounded-full text-xs font-medium",
                  "border transition",
                  availableCharacters.length === 0
                    ? "border-white/5 bg-white/5 text-white/30 cursor-not-allowed"
                    : "border-emerald-400/30 bg-emerald-400/10 text-emerald-300 hover:bg-emerald-400/20",
                )}
              >
                <Plus size={14} />
                Add
              </button>
            </div>

            <div className="space-y-2">
              <AnimatePresence mode="popLayout">
                {groupCharacters.map((character) => {
                  const percent = getParticipationPercent(character.id);

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
                        "border border-white/10 bg-[#0c0d13]/85",
                      )}
                    >
                      <CharacterAvatar character={character} size="md" />
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-white truncate">{character.name}</p>
                        <div className="flex items-center gap-2 mt-1">
                          <div className="flex-1 h-1.5 rounded-full bg-white/10 overflow-hidden">
                            <div
                              className="h-full bg-emerald-400/60 rounded-full transition-all duration-300"
                              style={{ width: `${percent}%` }}
                            />
                          </div>
                          <span className="text-[10px] text-white/50 tabular-nums">{percent}%</span>
                        </div>
                      </div>
                      <button
                        onClick={() => setShowRemoveConfirm(character.id)}
                        disabled={groupCharacters.length <= 2}
                        className={cn(
                          "flex items-center justify-center rounded-lg transition",
                          groupCharacters.length <= 2
                            ? "text-white/20 cursor-not-allowed"
                            : "text-white/40 hover:text-red-400 hover:bg-red-400/10",
                        )}
                        title={
                          groupCharacters.length <= 2
                            ? "Minimum 2 characters required"
                            : "Remove character"
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
              <p className="mt-2 text-xs text-white/40 text-center">
                A group chat requires at least 2 characters
              </p>
            )}
          </section>

          {/* Session Management */}
          <section className={spacing.item}>
            <SectionHeader
              title="Session Management"
              subtitle="Clone or branch this conversation"
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
                  "border-white/10 bg-[#0c0d13]/85 backdrop-blur-sm hover:border-white/20 hover:bg-white/10",
                )}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={cn(
                      "flex h-10 w-10 items-center justify-center",
                      radius.full,
                      "border border-white/15 bg-white/10 text-white/80",
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
                        "text-white/50",
                      )}
                    >
                      Clone Group
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-white truncate")}>
                      Duplicate this group with or without messages
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-white/30 transition-colors group-hover:text-white/60" />
              </button>

              <button
                onClick={() => setShowBranchOptions(true)}
                className={cn(
                  "group flex w-full min-h-14 items-center justify-between",
                  radius.md,
                  "border p-4 text-left",
                  interactive.transition.default,
                  interactive.active.scale,
                  "border-white/10 bg-[#0c0d13]/85 backdrop-blur-sm hover:border-white/20 hover:bg-white/10",
                )}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={cn(
                      "flex h-10 w-10 items-center justify-center",
                      radius.full,
                      "border border-white/15 bg-white/10 text-white/80",
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
                        "text-white/50",
                      )}
                    >
                      Branch with Character
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-white truncate")}>
                      Continue as 1-on-1 chat with a character
                    </div>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-white/30 transition-colors group-hover:text-white/60" />
              </button>
            </div>
          </section>

          {/* Participation Stats */}
          {participationStats.length > 0 && (
            <section className={spacing.item}>
              <SectionHeader
                title="Participation"
                subtitle="Speaking distribution across characters"
              />
              <div className={cn(radius.lg, "border border-white/10 bg-[#0c0d13]/85 p-4")}>
                {/* Visual bar */}
                <div className="h-3 rounded-full overflow-hidden flex bg-white/5 mb-4">
                  {groupCharacters.map((char, index) => {
                    const percent = getParticipationPercent(char.id);
                    const colors = [
                      "bg-emerald-400",
                      "bg-blue-400",
                      "bg-purple-400",
                      "bg-amber-400",
                      "bg-pink-400",
                      "bg-cyan-400",
                      "bg-orange-400",
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
                      "bg-emerald-400",
                      "bg-blue-400",
                      "bg-purple-400",
                      "bg-amber-400",
                      "bg-pink-400",
                      "bg-cyan-400",
                      "bg-orange-400",
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
                        <span className="text-xs text-white/70">{char.name}</span>
                        <span className="text-xs text-white/40 tabular-nums">({percent}%)</span>
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
        title="Add Character"
      >
        <div className="space-y-2 max-h-[60vh] overflow-y-auto">
          {availableCharacters.length === 0 ? (
            <div className="text-center py-8 text-white/50 text-sm">
              All characters are already in this group.
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
                  "border border-white/10 bg-[#0c0d13]/85",
                  interactive.transition.default,
                  "hover:border-white/20 hover:bg-white/10",
                  "disabled:opacity-50",
                )}
              >
                <CharacterAvatar character={character} size="md" />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-white truncate">{character.name}</p>
                  {(character.description || character.definition) && (
                    <p className="text-xs text-white/50 truncate mt-0.5">
                      {character.description || character.definition}
                    </p>
                  )}
                </div>
                <Plus className="h-4 w-4 text-emerald-400" />
              </button>
            ))
          )}
        </div>
      </BottomMenu>

      {/* Remove Character Confirmation */}
      <BottomMenu
        isOpen={showRemoveConfirm !== null}
        onClose={() => setShowRemoveConfirm(null)}
        title="Remove Character?"
      >
        {showRemoveConfirm && (
          <div className="space-y-4">
            <p className="text-sm text-white/70">
              Are you sure you want to remove{" "}
              <span className="font-medium text-white">
                {groupCharacters.find((c) => c.id === showRemoveConfirm)?.name}
              </span>{" "}
              from this group chat?
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setShowRemoveConfirm(null)}
                disabled={saving}
                className="flex-1 rounded-xl border border-white/10 bg-white/5 py-3 text-sm font-medium text-white transition hover:border-white/20 hover:bg-white/10 disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={() => handleRemoveCharacter(showRemoveConfirm)}
                disabled={saving}
                className="flex-1 rounded-xl border border-red-500/30 bg-red-500/20 py-3 text-sm font-medium text-red-300 transition hover:bg-red-500/30 disabled:opacity-50"
              >
                {saving ? "Removing..." : "Remove"}
              </button>
            </div>
          </div>
        )}
      </BottomMenu>

      {/* Clone Options Modal */}
      <BottomMenu
        isOpen={showCloneOptions}
        onClose={() => setShowCloneOptions(false)}
        title="Clone Group"
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
                "border-white/10 bg-[#0c0d13]/85 hover:border-white/20 hover:bg-white/10",
                cloning && "opacity-50 cursor-not-allowed",
              )}
            >
              <div className="flex items-center gap-3 min-w-0">
                <div
                  className={cn(
                    "flex h-10 w-10 items-center justify-center",
                    radius.full,
                    "border border-emerald-400/30 bg-emerald-400/10 text-emerald-300",
                  )}
                >
                  <Copy className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className={cn(typography.body.size, typography.body.weight, "text-white")}>
                    With messages
                  </p>
                  <p className={cn(typography.caption.size, "text-white/50 mt-0.5")}>
                    Clone everything including chat history
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
                "border-white/10 bg-[#0c0d13]/85 hover:border-white/20 hover:bg-white/10",
                cloning && "opacity-50 cursor-not-allowed",
              )}
            >
              <div className="flex items-center gap-3 min-w-0">
                <div
                  className={cn(
                    "flex h-10 w-10 items-center justify-center",
                    radius.full,
                    "border border-white/15 bg-white/10 text-white/80",
                  )}
                >
                  <Copy className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className={cn(typography.body.size, typography.body.weight, "text-white")}>
                    Without messages
                  </p>
                  <p className={cn(typography.caption.size, "text-white/50 mt-0.5")}>
                    Clone setup only (characters, starting scene)
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
        title="Branch with Character"
      >
        <MenuSection>
          <p className={cn(typography.bodySmall.size, "text-white/60 mb-3 px-1")}>
            Select a character to continue as a 1-on-1 conversation. All messages from this group
            will be converted.
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
                  "border-white/10 bg-[#0c0d13]/85 hover:border-white/20 hover:bg-white/10",
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
                        "text-white truncate",
                      )}
                    >
                      {character.name}
                    </p>
                    <p className={cn(typography.caption.size, "text-white/50 mt-0.5 truncate")}>
                      Continue conversation with {character.name}
                    </p>
                  </div>
                </div>
                <ChevronRight className="h-4 w-4 shrink-0 text-white/30 transition-colors group-hover:text-white/60" />
              </button>
            ))}
          </div>
        </MenuSection>
      </BottomMenu>
    </div>
  );
}
