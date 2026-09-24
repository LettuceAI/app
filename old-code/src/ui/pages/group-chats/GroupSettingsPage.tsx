import {
  ArrowLeft,
  User,
  Plus,
  Trash2,
  Edit2,
  Check,
  X,
  Image as ImageIcon,
  Brain,
  BarChart3,
  RefreshCw,
  Volume2,
  VolumeX,
  BookOpen,
  Clapperboard,
} from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import React, { useState } from "react";

import { processBackgroundImage } from "../../../core/utils/image";
import { useI18n } from "../../../core/i18n/context";
import { BottomMenu } from "../../components";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { useNavigationManager, Routes } from "../../navigation";
import { useAvatar } from "../../hooks/useAvatar";
import { AvatarImage } from "../../components/AvatarImage";
import { useGroupSettingsController } from "./hooks/useGroupSettingsController";
import {
  CharacterAvatar,
  GroupCharacterModelsSection,
  GroupPromptTemplateSection,
  PersonaSelector,
  QuickChip,
  SectionHeader,
} from "./components/settings";
import { OptionRow } from "./components/OptionRow";
import { Switch } from "../../components/Switch";

export function GroupSettingsPage() {
  const { t } = useI18n();
  const { groupId } = useParams<{ groupId: string }>();
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();

  const {
    group,
    personas,
    currentPersona,
    groupCharacters,
    availableCharacters,
    currentPersonaDisplay,
    mutedCharacterIds,
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
    handleSetCharacterMuted,
    handleChangeSpeakerSelectionMethod,
    handleChangeMemoryType,
    handleChangeCharacterModel,
    handleChangePromptTemplate,
    handleUpdateBackgroundImage,
    handleSetDisableCharacterLorebooks,
  } = useGroupSettingsController(groupId);

  const [backgroundImagePath, setBackgroundImagePath] = useState("");
  const [savingBackground, setSavingBackground] = useState(false);

  const personaAvatarUrl = useAvatar(
    "persona",
    currentPersona?.id ?? "",
    currentPersona?.avatarPath,
    "round",
  );

  React.useEffect(() => {
    if (group?.backgroundImagePath !== undefined) {
      setBackgroundImagePath(group.backgroundImagePath || "");
    }
  }, [group?.backgroundImagePath]);

  const handleBackgroundImageUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

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

  const handleBack = () => backOrReplace(Routes.groupChats);

  // Loading skeleton
  if (loading && !group) {
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
  if (error || !group) {
    return (
      <div className="flex h-full flex-col items-center justify-center text-fg p-8">
        <p className="text-lg font-medium text-danger">
          {error || t("groupChats.groupSettingsController.notFound")}
        </p>
        <button
          onClick={() => navigate(Routes.groupChats)}
          className="mt-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-2 text-sm"
        >
          {t("groupChats.groupSettingsExtra.backToGroups")}
        </button>
      </div>
    );
  }

  return (
    <div className="relative flex h-full flex-col text-fg bg-surface">
      {/* Header */}
      <header className="relative z-20 shrink-0 border-b border-fg/10 bg-surface px-4 pb-3 pt-[calc(env(safe-area-inset-top)+12px)]">
        <div className="flex items-center gap-3">
          <button
            onClick={handleBack}
            className="flex shrink-0 px-[0.6em] py-[0.3em] items-center justify-center -ml-2 text-fg transition hover:text-fg/80"
            aria-label={t("groupChats.groupSettingsExtra.backAria")}
          >
            <ArrowLeft size={14} strokeWidth={2.5} />
          </button>
          <div className="min-w-0 flex-1 text-left">
            <p className="truncate text-xl font-bold text-fg/90">
              {t("groupChats.groupSettings.title")}
            </p>
            <p className="mt-0.5 truncate text-xs text-fg/50">
              {t("groupChats.groupSettings.subtitle")}
            </p>
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
          {/* Header Card — Name + Background */}
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
                    alt="Background"
                    className="h-full w-full object-cover"
                  />
                  <div className="absolute inset-0 bg-linear-to-t from-surface-el/90 to-transparent" />
                  <div className="absolute right-2 top-2 flex items-center gap-1.5">
                    <label
                      title={t("groupChats.groupSettings.changeBackground")}
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
                      onClick={() => void handleRemoveBackground()}
                      disabled={savingBackground}
                      className={cn(
                        "flex h-7 w-7 items-center justify-center",
                        radius.full,
                        "bg-surface-el/60 text-fg/70 backdrop-blur-sm",
                        interactive.transition.fast,
                        "hover:bg-danger/80 hover:text-fg",
                        "disabled:opacity-50",
                      )}
                      aria-label={t("groupChats.groupSettings.removeBackground")}
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
                          placeholder={t("groupChats.groupSettings.enterGroupName")}
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
                            setNameDraft(group.name);
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
                            {group.name}
                          </p>
                          <p className={cn(typography.caption.size, "text-fg/45 mt-0.5")}>
                            {groupCharacters.length}{" "}
                            {groupCharacters.length === 1
                              ? t("groupChats.groupSettings.participant")
                              : t("groupChats.groupSettings.participants")}
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
                        ? t("groupChats.groupSettings.uploading")
                        : t("groupChats.groupSettings.addBackgroundImage")}
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
              </div>
            </div>
          </section>

          <p className={cn(typography.caption.size, "px-1 text-fg/45")}>
            {t("groupChats.groupSettings.defaultsHint")}
          </p>

          {/* Quick Settings */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.quickSettings")}
              subtitle={t("chats.settings.quickSettingsDesc")}
            />
            <div className="grid grid-cols-1 gap-2">
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
                label={t("groupChats.groupSettings.personaLabel")}
                value={currentPersonaDisplay}
                onClick={() => setShowPersonaSelector(true)}
              />
              <QuickChip
                icon={<BookOpen className="h-4 w-4" />}
                label={t("groupChats.groupSettingsPageExtra.manageLorebooks")}
                value={t("groupChats.groupSettings.lorebooksAttached", {
                  count: String(group.lorebookIds?.length ?? 0),
                })}
                onClick={() => navigate(Routes.groupLorebook(group.id))}
              />
            </div>
            <div
              className={cn(
                "flex w-full items-center justify-between gap-3 rounded-xl border border-fg/10 bg-surface-el/85 px-4 py-3 text-left",
                interactive.transition.default,
              )}
            >
              <div className="min-w-0">
                <div className="text-sm font-medium text-fg">
                  {t("groupChats.groupSettings.disableCharacterLorebooks")}
                </div>
                <div className="mt-0.5 text-xs text-fg/50">
                  {t("groupChats.groupSettings.disableCharacterLorebooksDesc")}
                </div>
              </div>
              <Switch
                checked={!!group.disableCharacterLorebooks}
                onChange={(next) => void handleSetDisableCharacterLorebooks(next)}
              />
            </div>
          </section>

          {/* Speaker Selection Method */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.groupSettings.speakerSelection")}
              subtitle={t("groupChats.groupSettings.speakerSubtitle")}
            />
            <div className="space-y-2">
              <OptionRow
                selected={group.speakerSelectionMethod === "llm"}
                onSelect={() => handleChangeSpeakerSelectionMethod("llm")}
                icon={Brain}
                label={t("groupChats.groupSettings.llm")}
                description={t("groupChats.groupSettings.llmDesc")}
                disabled={saving}
              />
              <OptionRow
                selected={group.speakerSelectionMethod === "heuristic"}
                onSelect={() => handleChangeSpeakerSelectionMethod("heuristic")}
                icon={BarChart3}
                label={t("groupChats.groupSettings.heuristic")}
                description={t("groupChats.groupSettings.heuristicDesc")}
                disabled={saving}
              />
              <OptionRow
                selected={group.speakerSelectionMethod === "round_robin"}
                onSelect={() => handleChangeSpeakerSelectionMethod("round_robin")}
                icon={RefreshCw}
                label={t("groupChats.groupSettings.roundRobin")}
                description={t("groupChats.groupSettings.roundRobinDesc")}
                disabled={saving}
              />
              <OptionRow
                selected={
                  group.speakerSelectionMethod === "director" ||
                  group.speakerSelectionMethod === "director_action"
                }
                onSelect={() => {
                  if (
                    group.speakerSelectionMethod !== "director" &&
                    group.speakerSelectionMethod !== "director_action"
                  ) {
                    handleChangeSpeakerSelectionMethod("director");
                  }
                }}
                icon={Clapperboard}
                label={t("groupChats.sessionSettings.director")}
                description={t("groupChats.create.groupSetup.directorRowDesc")}
                disabled={saving}
              />
              {(group.speakerSelectionMethod === "director" ||
                group.speakerSelectionMethod === "director_action") && (
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
                        group.speakerSelectionMethod === option.value
                          ? "border-accent/40 bg-accent/10"
                          : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
                        saving && "opacity-50",
                      )}
                    >
                      <div
                        className={cn(
                          "text-xs font-semibold",
                          group.speakerSelectionMethod === option.value
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
          </section>

          {/* Memory Mode */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("groupChats.groupSettings.memoryMode")}
              subtitle={t("groupChats.groupSettings.memorySubtitle")}
            />
            <div className="space-y-2">
              <OptionRow
                selected={group.memoryType === "manual"}
                onSelect={() => handleChangeMemoryType("manual")}
                icon={BookOpen}
                label={t("groupChats.groupSettings.manual")}
                description={t("groupChats.groupSettings.memoryManualInfo")}
                disabled={saving}
              />
              <OptionRow
                selected={group.memoryType === "dynamic"}
                onSelect={() => handleChangeMemoryType("dynamic")}
                icon={Brain}
                label={t("groupChats.groupSettings.dynamic")}
                description={t("groupChats.groupSettings.memoryDynamicInfo")}
                disabled={saving}
              />
            </div>
          </section>

          {/* Characters Section */}
          <section className={spacing.item}>
            <div className="flex items-center justify-between mb-3">
              <SectionHeader
                title={t("groupChats.groupSettings.characters")}
                subtitle={t("groupChats.groupSettings.participantsActive", {
                  total: String(groupCharacters.length),
                  active: String(groupCharacters.length - (group.mutedCharacterIds?.length ?? 0)),
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
                {t("groupChats.groupSettings.add")}
              </button>
            </div>

            <div className="space-y-2">
              <AnimatePresence mode="popLayout">
                {groupCharacters.map((character) => {
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
                              {t("groupChats.groupSettings.muted")}
                            </span>
                          )}
                        </p>
                        <p className="text-xs text-fg/50 mt-0.5">
                          {isMuted
                            ? t("groupChats.groupSettings.mutedByDefault")
                            : t("groupChats.groupSettings.activeByDefault")}
                        </p>
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
                            ? t("groupChats.groupSettings.unmuteCharacter")
                            : t("groupChats.groupSettings.muteCharacter")
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
                            ? t("groupChats.groupSettings.minTwoRequired")
                            : t("groupChats.groupSettings.removeCharacter")
                        }
                      >
                        <Trash2 size={14} />
                      </button>
                    </motion.div>
                  );
                })}
              </AnimatePresence>
            </div>

            <div className="mt-3 space-y-1.5 px-1">
              {groupCharacters.length <= 2 && (
                <p className={cn(typography.caption.size, "text-fg/45")}>
                  {t("groupChats.groupSettings.groupMinCharacters")}
                </p>
              )}
              <p className={cn(typography.caption.size, "text-fg/45")}>
                {t("groupChats.groupSettings.mutedCharactersNote")}
              </p>
            </div>
          </section>

          <section className={spacing.item}>
            <GroupCharacterModelsSection
              characters={groupCharacters}
              overrides={group.characterModelOverrides ?? {}}
              onChange={(characterId, modelId) =>
                void handleChangeCharacterModel(characterId, modelId)
              }
              disabled={saving}
            />
          </section>

          <section className={spacing.item}>
            <GroupPromptTemplateSection
              chatType={group.chatType}
              templateId={
                group.chatType === "roleplay"
                  ? (group.groupChatRoleplayPromptTemplateId ?? null)
                  : (group.groupChatPromptTemplateId ?? null)
              }
              onChange={(templateId) => void handleChangePromptTemplate(templateId)}
              disabled={saving}
            />
          </section>

          {/* Error banner */}
          {error ? (
            <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
              {error}
            </div>
          ) : null}
        </motion.div>
      </main>

      {/* Persona Selector Modal */}
      <PersonaSelector
        isOpen={showPersonaSelector}
        onClose={() => setShowPersonaSelector(false)}
        personas={personas}
        selectedPersonaId={group.personaId}
        onSelect={(personaId) => void handleChangePersona(personaId)}
      />

      {/* Add Character Modal */}
      <BottomMenu
        isOpen={showAddCharacter}
        onClose={() => setShowAddCharacter(false)}
        title={t("groupChats.groupSettings.addCharacterTitle")}
      >
        <div className="space-y-2 max-h-[60vh] overflow-y-auto">
          {availableCharacters.length === 0 ? (
            <div className="text-center py-8 text-fg/50 text-sm">
              {t("groupChats.groupSettings.allCharactersInGroup")}
            </div>
          ) : (
            availableCharacters.map((character) => (
              <button
                key={character.id}
                onClick={() => void handleAddCharacter(character.id)}
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
        title={t("groupChats.groupSettings.removeCharacterTitle")}
      >
        {showRemoveConfirm && (
          <div className="space-y-4">
            <p className="text-sm text-fg/70">
              {t("groupChats.groupSettings.removeCharacterConfirm")}{" "}
              <span className="font-medium text-fg">
                {groupCharacters.find((c) => c.id === showRemoveConfirm)?.name}
              </span>{" "}
              {t("groupChats.groupSettings.removeCharacterFrom")}
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
                  ? t("groupChats.groupSettings.removing")
                  : t("groupChats.groupSettings.remove")}
              </button>
            </div>
          </div>
        )}
      </BottomMenu>
    </div>
  );
}
