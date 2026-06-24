import React from "react";
import { useParams, useNavigate, useLocation } from "react-router-dom";
import {
  Loader2,
  RefreshCw,
  Plus,
  X,
  Sparkles,
  BookOpen,
  Cpu,
  Image,
  Download,
  Layers,
  Edit2,
  ChevronDown,
  Crop,
  Upload,
  User,
  Settings,
  Volume2,
  EyeOff,
  Check,
  Info,
  AlertTriangle,
  MessageSquare,
  ChevronRight,
  FolderOpen,
  Heart,
  Trash2,
  Users,
  Drama,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useEditCharacterForm } from "./hooks/useEditCharacterForm";
import { AvatarPicker } from "../../components/AvatarPicker";
import { DesignReferenceEditor } from "../../components/DesignReferenceEditor";
import { LoraSelector } from "../../components/LoraSelector";
import { CompanionSoulEditor } from "./components/CompanionSoulEditor";
import { CompanionScheduledNotesEditor } from "./components/CompanionScheduledNotesEditor";
import { SoulGenerationReviewOverlay } from "./components/SoulGenerationReviewOverlay";
import { normalizeCompanionConfig } from "./utils/companionDefaults";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuSection } from "../../components/BottomMenu";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { BackgroundPositionModal } from "../../components/BackgroundPositionModal";
import { CharacterExportMenu } from "../../components/CharacterExportMenu";
import { Switch } from "../../components/Switch";
import { ActiveLorebooksSelector } from "./components/ActiveLorebooksSelector";
import { InteractionModeSelector } from "./components/InteractionModeSelector";
import { cn, radius, colors, interactive, spacing, typography } from "../../design-tokens";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { useI18n } from "../../../core/i18n/context";
import type { CharacterFileFormat } from "../../../core/storage/characterTransfer";
import { convertFilePathToDataUrl } from "../../../core/storage/images";
import {
  insertSceneImageToken,
  storeSceneImageFromFile,
  storeSceneImageFromFilePath,
} from "../../../core/scene/inlineImages";
import {
  buildBackgroundLibrarySelectionKey,
  type BackgroundLibrarySelectionPayload,
} from "../../components/AvatarPicker/librarySelection";
import {
  listAudioProviders,
  listUserVoices,
  getProviderVoices,
  refreshProviderVoices,
  type AudioProvider,
  type CachedVoice,
  type UserVoice,
} from "../../../core/storage/audioProviders";
import {
  APP_COMPANION_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
} from "../../../core/prompts/constants";
import { generateCompanionSoulDraft } from "../../../core/companion/soul";
import { recalculateGradient } from "../../../core/storage/avatars";
import { useImageData } from "../../hooks/useImageData";
import { useAvatarGradient } from "../../hooks/useAvatarGradient";
import { toast } from "../../components/toast";
import { processBackgroundImage } from "../../../core/utils/image";

const wordCount = (text: string) => {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
};

const summarizeAvatarValue = (value?: string | null) => {
  if (!value) return "(empty)";
  if (value.startsWith("data:")) return `data-url(${value.slice(0, 24)}..., len=${value.length})`;
  return value.length > 96 ? `${value.slice(0, 96)}...` : value;
};

const GradientColorField = React.memo(function GradientColorField({
  label,
  value,
  placeholder,
  fallback,
  onCommit,
  onRemove,
}: {
  label: string;
  value?: string;
  placeholder: string;
  fallback: string;
  onCommit: (value: string) => void;
  onRemove?: () => void;
}) {
  const [draft, setDraft] = React.useState(value || "");

  React.useEffect(() => {
    setDraft(value || "");
  }, [value]);

  const commit = React.useCallback(
    (nextValue: string) => {
      if (nextValue !== (value || "")) {
        onCommit(nextValue);
      }
    },
    [onCommit, value],
  );

  return (
    <div className="flex items-center gap-3">
      <label className="w-12 text-xs text-fg/50">{label}</label>
      <div className="relative shrink-0">
        <input
          type="color"
          value={draft || fallback}
          onInput={(e) => {
            setDraft(e.currentTarget.value);
          }}
          onChange={(e) => {
            const nextValue = e.currentTarget.value;
            setDraft(nextValue);
            commit(nextValue);
          }}
          className="h-10 w-10 cursor-pointer rounded-lg border-2 border-fg/20 p-0.5"
          style={{
            backgroundColor: draft || fallback,
          }}
        />
      </div>
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => commit(draft)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.currentTarget.blur();
          }
        }}
        placeholder={placeholder}
        className="flex-1 rounded-lg border border-fg/10 bg-surface-el/50 px-3 py-2 text-sm font-mono text-fg placeholder:text-fg/30 focus:border-secondary/50 focus:outline-none"
      />
      {onRemove ? (
        <button
          type="button"
          onClick={onRemove}
          className="shrink-0 text-xs text-danger hover:text-danger"
        >
          ✕
        </button>
      ) : null}
    </div>
  );
});

type EditCharacterTab = "character" | "soul" | "settings";

const buildOpeningContext = (
  scenes: Array<{ title?: string | null; content: string; direction?: string | null }>,
) =>
  scenes
    .filter((scene) => scene.content.trim())
    .slice(0, 3)
    .map((scene, index) => {
      const title = scene.title?.trim() || `Scene ${index + 1}`;
      const direction = scene.direction?.trim();
      return direction
        ? `${title}\n${scene.content.trim()}\nDirection: ${direction}`
        : `${title}\n${scene.content.trim()}`;
    })
    .join("\n\n");

export function EditCharacterPage() {
  const { t } = useI18n();
  const { characterId } = useParams();
  const navigate = useNavigate();
  const location = useLocation();
  const { state, actions, computed } = useEditCharacterForm(characterId);
  const [expandedSceneId, setExpandedSceneId] = React.useState<string | null>(null);
  const [newSceneEditorOpen, setNewSceneEditorOpen] = React.useState(false);

  // Background image positioning state
  const [pendingBackgroundSrc, setPendingBackgroundSrc] = React.useState<string | null>(null);
  const [showBackgroundChoiceMenu, setShowBackgroundChoiceMenu] = React.useState(false);
  const [showBackgroundPositionModal, setShowBackgroundPositionModal] = React.useState(false);

  // Tab state
  const [activeTab, setActiveTab] = React.useState<EditCharacterTab>("character");
  const [generatingSoul, setGeneratingSoul] = React.useState(false);
  const [soulError, setSoulError] = React.useState<string | null>(null);
  const [soulDraft, setSoulDraft] = React.useState<Partial<import("../../../core/storage/schemas").CompanionConfig> | null>(null);
  const [soulDirection, setSoulDirection] = React.useState("");
  const [showModelMenu, setShowModelMenu] = React.useState(false);
  const [showVoiceMenu, setShowVoiceMenu] = React.useState(false);
  const [voiceSearchQuery, setVoiceSearchQuery] = React.useState("");
  const [exportMenuOpen, setExportMenuOpen] = React.useState(false);
  const [recalculatingGradient, setRecalculatingGradient] = React.useState(false);
  const sceneContentRef = React.useRef<HTMLTextAreaElement | null>(null);
  const sceneInlineImageInputRef = React.useRef<HTMLInputElement | null>(null);
  const sceneNavRestoredRef = React.useRef(false);
  const EDIT_CHARACTER_SCENE_DRAFT_KEY = "edit-character-scene-draft";
  const tabsId = React.useId();
  const tabPanelId = `${tabsId}-panel`;
  const characterTabId = `${tabsId}-tab-character`;
  const soulTabId = `${tabsId}-tab-soul`;
  const settingsTabId = `${tabsId}-tab-settings`;
  const returnPath = `${location.pathname}${location.search}`;
  const sceneBackgroundLibraryReturnPath = `${returnPath}:scene-background`;
  const sceneInlineImageLibraryReturnPath = `${returnPath}:scene-inline-image`;

  const {
    loading,
    saving,
    exporting,
    error,
    name,
    definition,
    description,
    nickname,
    creator,
    creatorNotes,
    creatorNotesMultilingualText,
    tagsText,
    avatarPath,
    avatarCrop,
    avatarRoundPath,
    avatarBannerPath,
    bannerCrop,
    cardType,
    designDescription,
    designReferenceImageIds,
    loraName,
    loraStrength,
    backgroundImagePath,
    scenes,
    chatTemplates,
    defaultSceneId,
    newSceneContent,
    newSceneDirection,
    newSceneBackgroundImagePath,
    selectedModelId,
    groupChatPromptTemplateId,
    groupChatRoleplayPromptTemplateId,
    activeLorebookIds,

    disableAvatarGradient,
    avatarGradientSource,
    customGradientEnabled,
    customGradientColors,
    customTextColor: _customTextColor,
    customTextSecondary: _customTextSecondary,
    memoryType,
    dynamicMemoryEnabled,
    models,
    loadingModels,
    promptTemplates,
    loadingTemplates,
    systemPromptTemplateId,
    companionPromptTemplateId,
    mode,
    voiceConfig,
    voiceAutoplay,
    companion,

    editingSceneId,
    editingSceneContent,
    editingSceneDirection,
    editingSceneBackgroundImagePath,
  } = state;

  const {
    setFields,
    handleSave,
    handleExport,
    addScene,
    deleteScene,
    startEditingScene,
    saveEditedScene,
    cancelEditingScene,
    resetToInitial,
  } = actions;

  const { avatarInitial, canSave } = computed;
  const directPromptTemplates = promptTemplates.filter(
    (template) => template.promptType === "undefined" || template.promptType === "directChat",
  );
  const groupChatTemplates = promptTemplates.filter(
    (template) =>
      template.promptType === "groupChatConversational" &&
      template.id !== APP_GROUP_CHAT_TEMPLATE_ID,
  );
  const groupChatRoleplayTemplates = promptTemplates.filter(
    (template) =>
      template.promptType === "groupChatRoleplay" &&
      template.id !== APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  );
  const companionPromptTemplates = promptTemplates.filter(
    (template) =>
      template.promptType === "companionChat" && template.id !== APP_COMPANION_TEMPLATE_ID,
  );
  const { colors: autoGradientColors, refreshGradient } = useAvatarGradient(
    "character",
    characterId ?? "",
    avatarPath ?? undefined,
    false,
    undefined,
    avatarGradientSource,
  );
  const suggestedCustomGradientColors = React.useMemo(() => {
    if (customGradientColors.length > 0) return customGradientColors;

    const detected = autoGradientColors
      .map((color) => color.hex)
      .filter((hex): hex is string => typeof hex === "string" && hex.length > 0)
      .slice(0, 3);

    return detected.length >= 2 ? detected : ["#4f46e5", "#7c3aed"];
  }, [autoGradientColors, customGradientColors]);
  const handleRecalculateGradient = React.useCallback(async () => {
    if (!characterId || !avatarPath || recalculatingGradient) return;

    setRecalculatingGradient(true);
    try {
      await recalculateGradient("character", characterId, avatarGradientSource);
      await refreshGradient(true);
      toast.success(
        t("characters.edit.gradientRecalculatedTitle"),
        t("characters.edit.gradientRecalculatedMessage"),
      );
    } catch (error) {
      console.error("Failed to recalculate avatar gradient:", error);
      toast.error(
        t("characters.edit.gradientRecalculateFailedTitle"),
        t("characters.edit.gradientRecalculateFailedMessage"),
      );
    } finally {
      setRecalculatingGradient(false);
    }
  }, [avatarGradientSource, avatarPath, characterId, recalculatingGradient, refreshGradient, t]);

  React.useEffect(() => {
    console.log("[EditCharacter] avatar state", {
      avatarPath: summarizeAvatarValue(avatarPath),
      avatarRoundPath: summarizeAvatarValue(avatarRoundPath),
    });
  }, [avatarPath, avatarRoundPath]);
  const tabItems = React.useMemo(
    () =>
      [
        { id: "character" as const, icon: User, label: t("characters.edit.tabCharacter"), disabled: false, hint: undefined as string | undefined },
        mode === "companion"
          ? {
              id: "soul" as const,
              icon: Heart,
              label: t("characters.edit.tabSoul"),
              disabled: false,
              hint: undefined as string | undefined,
            }
          : null,
        { id: "settings" as const, icon: Settings, label: t("characters.edit.tabSettings"), disabled: false, hint: undefined as string | undefined },
      ].filter(
        (item): item is {
          id: EditCharacterTab;
          icon: typeof User;
          label: string;
          disabled: boolean;
          hint: string | undefined;
        } => Boolean(item),
      ),
    [mode, t],
  );
  const activeTabId =
    activeTab === "character" ? characterTabId : activeTab === "soul" ? soulTabId : settingsTabId;

  const closeNewSceneEditor = React.useCallback(() => {
    setFields({
      newSceneContent: "",
      newSceneDirection: "",
      newSceneBackgroundImagePath: "",
    });
    setNewSceneEditorOpen(false);
  }, [setFields]);

  const saveNewScene = React.useCallback(() => {
    if (!newSceneContent.trim()) return;
    addScene();
    setNewSceneEditorOpen(false);
  }, [addScene, newSceneContent]);

  const sceneBackgroundInputRef = React.useRef<HTMLInputElement | null>(null);

  const handleSceneBackgroundUpload = React.useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      const input = event.target;
      if (!file) return;

      try {
        const dataUrl = await processBackgroundImage(file);
        setFields(
          editingSceneId !== null
            ? { editingSceneBackgroundImagePath: dataUrl }
            : { newSceneBackgroundImagePath: dataUrl },
        );
      } catch (error) {
        console.warn("EditCharacter: failed to process scene background image", error);
      } finally {
        input.value = "";
      }
    },
    [editingSceneId, setFields],
  );

  const handleInlineImageUpload = React.useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      const input = event.target;
      if (!file) return;
      const el = sceneContentRef.current;
      const cursor = el?.selectionStart ?? el?.value.length ?? 0;
      const current = el?.value ?? "";
      try {
        const stored = await storeSceneImageFromFile(file);
        if (!stored) return;
        const { content, nextCursor } = insertSceneImageToken(
          current,
          cursor,
          stored.imageId,
          stored.ext,
        );
        setFields(
          editingSceneId !== null
            ? { editingSceneContent: content }
            : { newSceneContent: content },
        );
        requestAnimationFrame(() => {
          const target = sceneContentRef.current;
          if (target) {
            target.focus();
            target.setSelectionRange(nextCursor, nextCursor);
          }
        });
      } catch (error) {
        console.warn("EditCharacter: failed to store inline scene image", error);
      } finally {
        input.value = "";
      }
    },
    [editingSceneId, setFields],
  );

  const buildSceneDraft = (extra: Record<string, unknown>) => ({
    characterId,
    editingSceneId,
    editingSceneContent,
    editingSceneDirection,
    editingSceneBackgroundImagePath,
    newSceneContent,
    newSceneDirection,
    newSceneBackgroundImagePath,
    newSceneEditorOpen,
    ...extra,
  });

  const persistSceneDraft = (extra: Record<string, unknown>) => {
    sceneNavRestoredRef.current = false;
    try {
      sessionStorage.setItem(
        EDIT_CHARACTER_SCENE_DRAFT_KEY,
        JSON.stringify(buildSceneDraft(extra)),
      );
    } catch (error) {
      console.error("Failed to persist scene editor draft:", error);
    }
  };

  const handleChooseInlineImageFromLibrary = () => {
    const el = sceneContentRef.current;
    const cursor = el?.selectionStart ?? el?.value.length ?? 0;
    persistSceneDraft({
      inlineImageTarget: editingSceneId !== null ? "edit" : "new",
      inlineImageCursor: cursor,
    });
    navigate("/library/images/pick", {
      state: {
        returnPath,
        selectionStorageKey: sceneInlineImageLibraryReturnPath,
        selectionKind: "background",
      },
    });
  };

  const handleExportFormat = React.useCallback(
    async (format: CharacterFileFormat) => {
      await handleExport(format);
      setExportMenuOpen(false);
    },
    [handleExport],
  );

  const soulGenerationDisabledReason = React.useMemo<string | null>(() => {
    if (!name.trim()) return t("characters.companionSoul.addNameFirst");
    if (!definition.trim()) return t("characters.companionSoul.addDefinitionFirst");
    return null;
  }, [name, definition, t]);

  const soulModelLabel = React.useMemo(() => {
    if (!selectedModelId) return null;
    return models.find((m) => m.id === selectedModelId)?.displayName ?? null;
  }, [selectedModelId, models]);

  const handleGenerateSoul = React.useCallback(async () => {
    if (soulGenerationDisabledReason || generatingSoul) {
      if (soulGenerationDisabledReason) setSoulError(soulGenerationDisabledReason);
      return;
    }
    setGeneratingSoul(true);
    setSoulError(null);
    try {
      const draft = await generateCompanionSoulDraft({
        characterName: name.trim(),
        characterDefinition: definition,
        characterDescription: description,
        openingContext: buildOpeningContext(scenes),
        currentSoul: companion,
        userNotes: soulDirection.trim() || null,
        modelId: selectedModelId,
      });
      setSoulDraft(draft);
    } catch (err) {
      console.error("Failed to generate companion soul:", err);
      setSoulError(err instanceof Error ? err.message : t("characters.edit.soulGenerateFailed"));
    } finally {
      setGeneratingSoul(false);
    }
  }, [
    companion,
    definition,
    description,
    generatingSoul,
    name,
    scenes,
    selectedModelId,
    soulDirection,
    soulGenerationDisabledReason,
    t,
  ]);

  const handleApplySoulDraft = React.useCallback(
    (next: import("../../../core/storage/schemas").CompanionConfig) => {
      setFields({ companion: next });
      setSoulDraft(null);
    },
    [setFields],
  );

  const [audioProviders, setAudioProviders] = React.useState<AudioProvider[]>([]);
  const [userVoices, setUserVoices] = React.useState<UserVoice[]>([]);
  const [providerVoices, setProviderVoices] = React.useState<Record<string, CachedVoice[]>>({});
  const [loadingVoices, setLoadingVoices] = React.useState(false);
  const [voiceError, setVoiceError] = React.useState<string | null>(null);
  const [hasLoadedVoices, setHasLoadedVoices] = React.useState(false);

  const buildUserVoiceValue = (id: string) => `user:${id}`;
  const buildProviderVoiceValue = (providerId: string, voiceId: string) =>
    `provider:${providerId}:${voiceId}`;

  const voiceSelectionValue = (() => {
    if (!voiceConfig) return "";
    if (voiceConfig.source === "user" && voiceConfig.userVoiceId) {
      return buildUserVoiceValue(voiceConfig.userVoiceId);
    }
    if (voiceConfig.source === "provider" && voiceConfig.providerId && voiceConfig.voiceId) {
      return buildProviderVoiceValue(voiceConfig.providerId, voiceConfig.voiceId);
    }
    return "";
  })();

  React.useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__saveCharacter = handleSave;
    globalWindow.__saveCharacterCanSave = canSave;
    globalWindow.__saveCharacterSaving = saving;
    return () => {
      delete globalWindow.__saveCharacter;
      delete globalWindow.__saveCharacterCanSave;
      delete globalWindow.__saveCharacterSaving;
    };
  }, [handleSave, canSave, saving]);

  React.useEffect(() => {
    const handleDiscard = () => resetToInitial();
    window.addEventListener("unsaved:discard", handleDiscard);
    return () => window.removeEventListener("unsaved:discard", handleDiscard);
  }, [resetToInitial]);

  React.useEffect(() => {
    if (loading) return;

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
      setFields({ backgroundImagePath: dataUrl });
    })();

    return () => {
      cancelled = true;
    };
  }, [loading, returnPath, setFields]);

  React.useEffect(() => {
    if (loading || sceneNavRestoredRef.current) return;

    const draftRaw = sessionStorage.getItem(EDIT_CHARACTER_SCENE_DRAFT_KEY);
    const bgKey = buildBackgroundLibrarySelectionKey(sceneBackgroundLibraryReturnPath);
    const inlineKey = buildBackgroundLibrarySelectionKey(sceneInlineImageLibraryReturnPath);
    const bgRaw = sessionStorage.getItem(bgKey);
    const inlineRaw = sessionStorage.getItem(inlineKey);

    if (!draftRaw && !bgRaw && !inlineRaw) return;
    sceneNavRestoredRef.current = true;

    sessionStorage.removeItem(EDIT_CHARACTER_SCENE_DRAFT_KEY);
    sessionStorage.removeItem(bgKey);
    sessionStorage.removeItem(inlineKey);

    let draft: any = null;
    if (draftRaw) {
      try {
        draft = JSON.parse(draftRaw);
      } catch (error) {
        console.error("Failed to parse scene editor draft:", error);
      }
    }
    if (draft && draft.characterId && draft.characterId !== characterId) return;

    const parseSelection = (raw: string | null): BackgroundLibrarySelectionPayload | null => {
      if (!raw) return null;
      try {
        const parsed = JSON.parse(raw) as BackgroundLibrarySelectionPayload;
        return parsed?.filePath ? parsed : null;
      } catch {
        return null;
      }
    };
    const inlineSel = parseSelection(inlineRaw);
    const bgSel = parseSelection(bgRaw);

    let cancelled = false;
    void (async () => {
      const next: Parameters<typeof setFields>[0] = {};
      if (draft) {
        next.editingSceneId = draft.editingSceneId ?? null;
        next.editingSceneContent = draft.editingSceneContent ?? "";
        next.editingSceneDirection = draft.editingSceneDirection ?? "";
        next.editingSceneBackgroundImagePath = draft.editingSceneBackgroundImagePath ?? "";
        next.newSceneContent = draft.newSceneContent ?? "";
        next.newSceneDirection = draft.newSceneDirection ?? "";
        next.newSceneBackgroundImagePath = draft.newSceneBackgroundImagePath ?? "";
      }
      const editing = (draft ? draft.editingSceneId : editingSceneId) !== null;

      if (inlineSel) {
        const stored = await storeSceneImageFromFilePath(inlineSel.filePath);
        if (stored && !cancelled) {
          const target = (draft?.inlineImageTarget ?? (editing ? "edit" : "new")) as "edit" | "new";
          const cursor = typeof draft?.inlineImageCursor === "number" ? draft.inlineImageCursor : 0;
          const baseContent =
            target === "edit"
              ? next.editingSceneContent ?? editingSceneContent
              : next.newSceneContent ?? newSceneContent;
          const { content } = insertSceneImageToken(
            baseContent,
            cursor,
            stored.imageId,
            stored.ext,
          );
          if (target === "edit") next.editingSceneContent = content;
          else next.newSceneContent = content;
        }
      }

      if (bgSel) {
        const dataUrl = await convertFilePathToDataUrl(bgSel.filePath);
        if (dataUrl && !cancelled) {
          const bgTarget = (draft?.sceneBackgroundTarget ?? (editing ? "edit" : "new")) as
            | "edit"
            | "new";
          if (bgTarget === "edit") next.editingSceneBackgroundImagePath = dataUrl;
          else next.newSceneBackgroundImagePath = dataUrl;
        }
      }

      if (cancelled) return;
      if (Object.keys(next).length > 0) setFields(next);
      if (draft?.newSceneEditorOpen) setNewSceneEditorOpen(true);
    })();

    return () => {
      cancelled = true;
    };
  }, [
    loading,
    characterId,
    sceneBackgroundLibraryReturnPath,
    sceneInlineImageLibraryReturnPath,
    editingSceneId,
    editingSceneContent,
    newSceneContent,
    setFields,
  ]);

  const handleChooseBackgroundFromLibrary = React.useCallback(() => {
    navigate("/library/images/pick", {
      state: {
        returnPath,
        selectionKind: "background",
      },
    });
  }, [navigate, returnPath]);

  const handleChooseSceneBackgroundFromLibrary = (target: "new" | "edit") => {
    persistSceneDraft({ sceneBackgroundTarget: target });
    navigate("/library/images/pick", {
      state: {
        returnPath,
        selectionStorageKey: sceneBackgroundLibraryReturnPath,
        selectionKind: "background",
      },
    });
  };

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
            if (
              (provider.providerType === "elevenlabs" || provider.providerType === "fish_tts") &&
              provider.apiKey
            ) {
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
      setVoiceError(t("characters.voiceLoading.failed"));
    } finally {
      setLoadingVoices(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (activeTab !== "settings" || hasLoadedVoices) return;
    void loadVoices();
  }, [activeTab, hasLoadedVoices, loadVoices]);

  React.useEffect(() => {
    if (mode !== "companion" && activeTab === "soul") {
      setActiveTab("settings");
    }
  }, [activeTab, mode]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-white/60" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col pb-16 text-fg/80">
      <main
        id={tabPanelId}
        role="tabpanel"
        aria-labelledby={activeTabId}
        tabIndex={0}
        className="flex-1 overflow-y-auto px-4"
      >
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="space-y-5 pb-6 pt-4"
        >
          {/* Character Tab Content */}
          {activeTab === "character" && (
            <>
              {/* Error Message */}
              <AnimatePresence>
                {error && (
                  <motion.div
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -10 }}
                    transition={{ duration: 0.15 }}
                    className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3"
                  >
                    <p className="text-sm text-danger">{error}</p>
                  </motion.div>
                )}
              </AnimatePresence>
            </>
          )}

          {/* Settings Tab Content */}
          {activeTab === "settings" && (
            <>
              <InteractionModeSelector
                mode={mode}
                onChange={(nextMode) => setFields({ mode: nextMode })}
                disabled={saving}
              />

              {/* Background Image Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
                    <Image className="h-4 w-4 text-secondary" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("characters.edit.chatBackgroundTitle")}</h3>
                  <span className="text-xs text-fg/40">{t("characters.edit.optionalSuffix")}</span>
                </div>

                <div className="overflow-hidden rounded-xl border border-fg/10 bg-surface-el/20">
                  {backgroundImagePath ? (
                    <div className="relative">
                      <img
                        src={backgroundImagePath}
                        alt={t("characters.edit.backgroundPreviewAlt")}
                        className="h-32 w-full object-cover"
                      />
                      <div className="absolute inset-0 bg-surface-el/30 flex items-center justify-center">
                        <span className="text-xs text-fg/80 bg-surface-el/50 px-2 py-1 rounded">
                          {t("characters.edit.backgroundPreviewBadge")}
                        </span>
                      </div>
                      <button
                        type="button"
                        onClick={() => setFields({ backgroundImagePath: "" })}
                        className="absolute top-2 right-2 rounded-full border border-fg/20 bg-surface-el/50 p-1 text-fg/70 transition hover:bg-surface-el/70 active:scale-95"
                        aria-label={t("characters.edit.removeBackgroundImage")}
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ) : (
                    <div className="flex h-32 flex-col items-center justify-center gap-2">
                      <div className="rounded-lg border border-fg/10 bg-fg/5 p-2">
                        <Image size={20} className="text-fg/40" />
                      </div>
                      <div className="text-center">
                        <p className="text-sm text-fg/70">{t("characters.edit.addBackgroundImage")}</p>
                        <p className="text-xs text-fg/40">{t("characters.edit.addBackgroundHint")}</p>
                      </div>
                    </div>
                  )}
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <label className="flex cursor-pointer items-center justify-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-3 text-sm text-fg/75 transition hover:bg-surface-el/30">
                    <Upload size={14} />
                    {t("characters.edit.uploadImage")}
                    <input
                      type="file"
                      accept="image/*"
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (!file) return;
                        const reader = new FileReader();
                        reader.onload = () => {
                          const dataUrl = reader.result as string;
                          setPendingBackgroundSrc(dataUrl);
                          setShowBackgroundChoiceMenu(true);
                        };
                        reader.readAsDataURL(file);
                        e.target.value = "";
                      }}
                      className="hidden"
                    />
                  </label>
                  <button
                    type="button"
                    onClick={handleChooseBackgroundFromLibrary}
                    className="flex items-center justify-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-3 text-sm text-fg/75 transition hover:bg-surface-el/30"
                  >
                    <FolderOpen size={14} />
                    {t("characters.edit.chooseFromLibrary")}
                  </button>
                </div>
                <p className="text-xs text-fg/50">
                  {t("characters.edit.chatBackgroundHint")}
                </p>
              </div>

              {avatarPath && (
                <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
                  {/* Avatar Gradient Toggle */}
                  <div className="space-y-3">
                    <div className="flex items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 transition hover:bg-surface-el/30">
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <Sparkles className="h-4 w-4 text-accent" />
                          <p className="text-sm font-medium text-fg">{t("characters.edit.avatarGradientTitle")}</p>
                        </div>
                        <p className="mt-0.5 text-xs text-fg/50">
                          {t("characters.edit.avatarGradientDesc")}
                        </p>
                      </div>
                      <div className="ml-3 flex items-center gap-2">
                        <button
                          type="button"
                          onClick={handleRecalculateGradient}
                          disabled={recalculatingGradient}
                          className={cn(
                            "inline-flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/65 transition",
                            recalculatingGradient
                              ? "cursor-wait opacity-70"
                              : "hover:bg-fg/10 hover:text-fg active:scale-95",
                          )}
                          aria-label={t("characters.edit.recalculateGradientAria")}
                          title={t("characters.edit.recalculateGradientAria")}
                        >
                          {recalculatingGradient ? (
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                          ) : (
                            <RefreshCw className="h-3.5 w-3.5" />
                          )}
                        </button>
                        <Switch
                          checked={!disableAvatarGradient}
                          onChange={(next) => setFields({ disableAvatarGradient: !next })}
                        />
                      </div>
                    </div>
                    <AnimatePresence initial={false}>
                      {!disableAvatarGradient && (
                        <motion.div
                          initial={{ opacity: 0, y: -6 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={{ opacity: 0, y: -6 }}
                          transition={{ duration: 0.18, ease: "easeOut" }}
                          className="rounded-lg border border-fg/10 bg-surface-el/10 p-2"
                        >
                          <div className="mb-2 flex items-center justify-between">
                            <span className="text-xs font-medium text-fg/75">{t("characters.edit.gradientSourceLabel")}</span>
                            <span className="text-[11px] text-fg/45">
                              {t("characters.edit.gradientSourceHint")}
                            </span>
                          </div>
                          <div className="grid grid-cols-2 gap-2">
                            <button
                              type="button"
                              onClick={() => setFields({ avatarGradientSource: "base" })}
                              className={cn(
                                "rounded-lg border px-3 py-2 text-sm transition",
                                avatarGradientSource === "base"
                                  ? "border-accent/40 bg-accent/12 text-accent"
                                  : "border-fg/10 bg-fg/5 text-fg/70 hover:bg-fg/10",
                              )}
                            >
                              {t("characters.edit.gradientSourceBase")}
                            </button>
                            <button
                              type="button"
                              onClick={() => setFields({ avatarGradientSource: "round" })}
                              className={cn(
                                "rounded-lg border px-3 py-2 text-sm transition",
                                avatarGradientSource === "round"
                                  ? "border-accent/40 bg-accent/12 text-accent"
                                  : "border-fg/10 bg-fg/5 text-fg/70 hover:bg-fg/10",
                              )}
                            >
                              {t("characters.edit.gradientSourceCropped")}
                            </button>
                          </div>
                        </motion.div>
                      )}
                      {customGradientEnabled && !disableAvatarGradient && (
                        <motion.div
                          initial={{ opacity: 0, y: -6 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={{ opacity: 0, y: -6 }}
                          transition={{ duration: 0.18, ease: "easeOut" }}
                          className="rounded-lg border border-warning/20 bg-warning/8 px-2.5 py-2"
                        >
                          <div className="flex min-h-4 items-center gap-2 text-xs text-warning/85">
                            <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
                            <span className="block leading-none">
                              {t("characters.edit.customGradientOverrideWarning")}
                            </span>
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>

                  {/* Custom Gradient Override */}
                  <div className="space-y-3">
                    <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <div className="flex items-center justify-between">
                        <div className="flex-1">
                          <div className="flex items-center gap-2">
                            <Sparkles className="h-4 w-4 text-secondary" />
                            <p className="text-sm font-medium text-fg">{t("characters.edit.customGradientTitle")}</p>
                          </div>
                          <p className="mt-0.5 text-xs text-fg/50">
                            {t("characters.edit.customGradientDesc")}
                          </p>
                        </div>
                        <div className="ml-3">
                          <Switch
                            checked={customGradientEnabled}
                            onChange={(next) => {
                              if (next) {
                                setFields({
                                  customGradientEnabled: true,
                                  customGradientColors: suggestedCustomGradientColors,
                                });
                              } else {
                                setFields({ customGradientEnabled: false });
                              }
                            }}
                          />
                        </div>
                      </div>

                      {customGradientEnabled && (
                        <div className="mt-4 space-y-3 border-t border-fg/10 pt-4">
                          <div
                            className="h-16 w-full rounded-lg"
                            style={{
                              background:
                                suggestedCustomGradientColors.length >= 3
                                  ? `linear-gradient(135deg, ${suggestedCustomGradientColors[0]}, ${suggestedCustomGradientColors[2]}, ${suggestedCustomGradientColors[1]})`
                                  : suggestedCustomGradientColors.length >= 2
                                    ? `linear-gradient(135deg, ${suggestedCustomGradientColors[0]}, ${suggestedCustomGradientColors[1]})`
                                    : suggestedCustomGradientColors[0],
                            }}
                          />

                          <GradientColorField
                            label={t("characters.edit.gradientColorStart")}
                            value={suggestedCustomGradientColors[0] || ""}
                            placeholder="#4f46e5"
                            fallback="#4f46e5"
                            onCommit={(nextValue) => {
                              const newColors = [...suggestedCustomGradientColors];
                              newColors[0] = nextValue;
                              setFields({ customGradientColors: newColors });
                            }}
                          />

                          {suggestedCustomGradientColors.length >= 3 ? (
                            <GradientColorField
                              label={t("characters.edit.gradientColorMid")}
                              value={suggestedCustomGradientColors[2] || ""}
                              placeholder="#a855f7"
                              fallback="#a855f7"
                              onCommit={(nextValue) => {
                                const newColors = [...suggestedCustomGradientColors];
                                newColors[2] = nextValue;
                                setFields({ customGradientColors: newColors });
                              }}
                              onRemove={() => {
                                const newColors = [
                                  suggestedCustomGradientColors[0],
                                  suggestedCustomGradientColors[1],
                                ];
                                setFields({ customGradientColors: newColors });
                              }}
                            />
                          ) : (
                            <button
                              type="button"
                              onClick={() => {
                                const newColors = [
                                  suggestedCustomGradientColors[0],
                                  suggestedCustomGradientColors[1],
                                  "#a855f7",
                                ];
                                setFields({ customGradientColors: newColors });
                              }}
                              className="py-1 text-xs text-secondary hover:text-secondary"
                            >
                              {t("characters.edit.addMiddleColor")}
                            </button>
                          )}

                          <GradientColorField
                            label={t("characters.edit.gradientColorEnd")}
                            value={suggestedCustomGradientColors[1] || ""}
                            placeholder="#7c3aed"
                            fallback="#7c3aed"
                            onCommit={(nextValue) => {
                              const newColors = [...suggestedCustomGradientColors];
                              newColors[1] = nextValue;
                              setFields({ customGradientColors: newColors });
                            }}
                          />

                          <p className="mt-2 text-[10px] text-fg/40">
                            {t("characters.edit.textColorsAutoHint")}
                          </p>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </>
          )}

          {activeTab === "soul" && mode === "companion" && (
            <div className="mx-auto w-full max-w-5xl space-y-4">
              <div className={spacing.tight}>
                <div className="flex items-center gap-2">
                  <div className={cn("border border-rose-400/30 bg-rose-500/10 p-1.5", radius.md)}>
                    <Heart className="h-4 w-4 text-rose-300" />
                  </div>
                  <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
                    {t("characters.edit.companionSoulTitle")}
                  </h2>
                </div>
                <p className={cn(typography.body.size, "text-fg/50")}>
                  {t("characters.edit.companionSoulSubtitle")}
                </p>
              </div>

              {soulError && (
                <div
                  className={cn(
                    "flex items-start gap-2 border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger",
                    radius.lg,
                  )}
                >
                  <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                  <span className="flex-1">{soulError}</span>
                  <button
                    type="button"
                    onClick={() => {
                      setSoulError(null);
                      void handleGenerateSoul();
                    }}
                    className={cn(
                      "border border-danger/30 bg-danger/15 px-2 py-1 text-xs font-medium text-danger hover:bg-danger/25",
                      radius.md,
                      interactive.transition.fast,
                    )}
                  >
                    {t("characters.companionSoul.retry")}
                  </button>
                </div>
              )}

              <CompanionSoulEditor
                companion={companion}
                onChange={(next) => setFields({ companion: next })}
                disabled={saving || generatingSoul}
                onGenerate={handleGenerateSoul}
                generating={generatingSoul}
                generationDisabledReason={soulGenerationDisabledReason}
                modelLabel={soulModelLabel}
                direction={soulDirection}
                onDirectionChange={setSoulDirection}
              />

              <SoulGenerationReviewOverlay
                isOpen={soulDraft !== null}
                baseline={normalizeCompanionConfig(companion)}
                draft={soulDraft}
                direction={soulDirection}
                onDirectionChange={setSoulDirection}
                onApply={handleApplySoulDraft}
                onCancel={() => setSoulDraft(null)}
                onRegenerate={handleGenerateSoul}
                regenerating={generatingSoul}
              />

              {characterId ? (
                <CompanionScheduledNotesEditor characterId={characterId} />
              ) : null}
            </div>
          )}

          {/* Character Tab: Personality & Scenes */}
          {activeTab === "character" && (
            <>
              <div className="grid grid-cols-1 gap-8 xl:grid-cols-[minmax(0,420px)_minmax(0,1fr)] xl:items-start">
                <div className="space-y-6 xl:sticky xl:top-4">
                  <div className="flex flex-col items-center py-3 xl:py-0">
                    <div className="relative">
                      <AvatarPicker
                        currentAvatarPath={avatarPath}
                        onAvatarChange={(path) => {
                          console.log("[EditCharacter] onAvatarChange", {
                            path: summarizeAvatarValue(path),
                          });
                          setFields({ avatarPath: path });
                        }}
                        promptSubjectName={name}
                        promptSubjectDescription={definition}
                        loraTag={loraName ? `<lora:${loraName}:${loraStrength ?? 0.8}>` : null}
                        avatarCrop={avatarCrop}
                        onAvatarCropChange={(crop) => setFields({ avatarCrop: crop })}
                        avatarRoundPath={avatarRoundPath}
                        onAvatarRoundChange={(roundPath) => {
                          console.log("[EditCharacter] onAvatarRoundChange", {
                            roundPath: summarizeAvatarValue(roundPath),
                          });
                          setFields({ avatarRoundPath: roundPath });
                        }}
                        placeholder={avatarInitial}
                      />

                      {avatarPath && (
                        <button
                          type="button"
                          onClick={() =>
                            setFields({
                              avatarPath: "",
                              avatarCrop: null,
                              avatarRoundPath: null,
                              avatarBannerPath: null,
                              bannerCrop: null,
                            })
                          }
                          className="absolute -top-1 -left-1 z-30 flex h-10 w-10 items-center justify-center rounded-full border border-fg/10 bg-surface-el text-fg/60 transition hover:bg-danger/80 hover:border-danger/50 hover:text-fg active:scale-95"
                          aria-label={t("common.buttons.remove")}
                        >
                          <X size={14} strokeWidth={2.5} />
                        </button>
                      )}
                    </div>
                    <p className="mt-3 text-center text-xs text-fg/40">
                      {t("characters.edit.tapToAddAvatar")}
                    </p>
                  </div>

                  <div className="space-y-4 rounded-2xl border border-fg/10 bg-surface-el/10 p-4">
                    <div>
                      <p className="text-sm font-medium text-fg">{t("characters.edit.cardTypeLabel")}</p>
                      <div className="mt-2 grid grid-cols-2 gap-2">
                        {(["circle", "banner"] as const).map((value) => (
                          <button
                            key={value}
                            type="button"
                            onClick={() => setFields({ cardType: value })}
                            className={cn(
                              "rounded-xl border px-3 py-2 text-sm font-medium transition",
                              cardType === value
                                ? "border-accent/50 bg-accent/10 text-fg"
                                : "border-fg/10 bg-surface-el/20 text-fg/60 hover:border-fg/20 hover:text-fg",
                            )}
                          >
                            {value === "circle"
                              ? t("characters.edit.cardTypeCircle")
                              : t("characters.edit.cardTypeBanner")}
                          </button>
                        ))}
                      </div>
                    </div>

                    <div>
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <p className="text-sm font-medium text-fg">{t("characters.edit.bannerImageLabel")}</p>
                          <p className="mt-1 text-xs text-fg/45">
                            {t("characters.edit.bannerImageHint")}
                          </p>
                        </div>
                        {avatarBannerPath ? (
                          <button
                            type="button"
                            onClick={() =>
                              setFields({ avatarBannerPath: null, bannerCrop: null })
                            }
                            className={cn(
                              "flex shrink-0 items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium",
                              "border-danger/25 bg-danger/10 text-danger/90",
                              "transition hover:border-danger/40 hover:bg-danger/15 active:scale-[0.98]",
                            )}
                            aria-label={t("characters.edit.removeBannerImage")}
                          >
                            <Trash2 size={12} strokeWidth={2.2} />
                            <span>{t("common.buttons.remove")}</span>
                          </button>
                        ) : null}
                      </div>

                      <div className="mt-3 flex flex-col items-center gap-2">
                        <AvatarPicker
                          currentAvatarPath={avatarBannerPath || ""}
                          onAvatarChange={(path) => setFields({ avatarBannerPath: path })}
                          librarySelectionScope="character-banner"
                          promptSubjectName={name}
                          promptSubjectDescription={definition}
                          loraTag={loraName ? `<lora:${loraName}:${loraStrength ?? 0.8}>` : null}
                          avatarCrop={bannerCrop}
                          onAvatarCropChange={(crop) => setFields({ bannerCrop: crop })}
                          shape="banner"
                          size="lg"
                        />
                        {!avatarBannerPath && (
                          <p className="text-[11px] text-fg/35">
                            {t("characters.edit.bannerImageTapHint")}
                          </p>
                        )}
                      </div>
                    </div>
                  </div>

                  <div className={spacing.field}>
                    <label
                      className={cn(
                        typography.label.size,
                        typography.label.weight,
                        typography.label.tracking,
                        "uppercase text-fg/70",
                      )}
                    >
                      {t("characters.edit.characterNameLabel")}
                    </label>
                    <input
                      value={name}
                      onChange={(e) => setFields({ name: e.target.value })}
                      placeholder={t("characters.edit.characterNamePlaceholder")}
                      className={cn(
                        "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        typography.body.size,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        name.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                      )}
                    />
                  </div>

                  <div className={spacing.field}>
                    <label
                      className={cn(
                        typography.label.size,
                        typography.label.weight,
                        typography.label.tracking,
                        "uppercase text-fg/70",
                      )}
                    >
                      {t("characters.edit.nicknameLabel")}
                    </label>
                    <input
                      value={nickname}
                      onChange={(e) => setFields({ nickname: e.target.value })}
                      placeholder={t("characters.edit.nicknamePlaceholder")}
                      className={cn(
                        "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        typography.body.size,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        nickname.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                      )}
                    />
                  </div>

                  <div className={spacing.field}>
                    <label
                      className={cn(
                        typography.label.size,
                        typography.label.weight,
                        typography.label.tracking,
                        "uppercase text-fg/70",
                      )}
                    >
                      {t("characters.edit.creatorLabel")}
                    </label>
                    <input
                      value={creator}
                      onChange={(e) => setFields({ creator: e.target.value })}
                      placeholder={t("characters.edit.creatorPlaceholder")}
                      className={cn(
                        "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        typography.body.size,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        creator.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                      )}
                    />
                  </div>

                  <div className={spacing.field}>
                    <label
                      className={cn(
                        typography.label.size,
                        typography.label.weight,
                        typography.label.tracking,
                        "uppercase text-fg/70",
                      )}
                    >
                      {t("characters.edit.tagsLabel")}
                    </label>
                    <textarea
                      value={tagsText}
                      onChange={(e) => setFields({ tagsText: e.target.value })}
                      rows={2}
                      placeholder={t("characters.edit.tagsPlaceholder")}
                      className={cn(
                        "w-full resize-none border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        typography.body.size,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        tagsText.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                      )}
                    />
                  </div>

                  <div className={spacing.field}>
                    <label
                      className={cn(
                        typography.label.size,
                        typography.label.weight,
                        typography.label.tracking,
                        "uppercase text-fg/70",
                      )}
                    >
                      {t("characters.edit.creatorNotesLabel")}
                    </label>
                    <textarea
                      value={creatorNotes}
                      onChange={(e) => setFields({ creatorNotes: e.target.value })}
                      rows={4}
                      placeholder={t("characters.edit.creatorNotesPlaceholder")}
                      className={cn(
                        "w-full resize-none border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        typography.body.size,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        creatorNotes.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                      )}
                    />
                  </div>

                  <div className={spacing.field}>
                    <label
                      className={cn(
                        typography.label.size,
                        typography.label.weight,
                        typography.label.tracking,
                        "uppercase text-fg/70",
                      )}
                    >
                      {t("characters.edit.creatorNotesMultilingualLabel")}
                    </label>
                    <textarea
                      value={creatorNotesMultilingualText}
                      onChange={(e) => setFields({ creatorNotesMultilingualText: e.target.value })}
                      rows={5}
                      placeholder='{"en":"note","ja":"メモ"}'
                      className={cn(
                        "w-full resize-none border bg-surface-el/20 px-4 py-3.5 font-mono text-xs text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        creatorNotesMultilingualText.trim()
                          ? "border-accent/30 bg-accent/5"
                          : "border-fg/10",
                      )}
                    />
                  </div>

                  <div className="border-t border-fg/10 pt-5">
                    <div className="space-y-3">
                      <button
                        type="button"
                        onClick={() => navigate(`/settings/characters/${characterId}/templates`)}
                        className="group flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:border-fg/25 hover:bg-surface-el/30 active:bg-surface-el/40"
                      >
                        <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
                          <MessageSquare className="h-4 w-4 text-secondary" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <div className="text-sm font-medium text-fg">{t("characters.edit.chatTemplatesTitle")}</div>
                            {(chatTemplates?.length ?? 0) > 0 && (
                              <span className="rounded-full border border-fg/10 bg-fg/5 px-2 py-0.5 text-xs text-fg/70">
                                {chatTemplates?.length ?? 0}
                              </span>
                            )}
                          </div>
                          <p className="mt-0.5 text-xs text-fg/50">
                            {(chatTemplates?.length ?? 0) > 0
                              ? t("characters.edit.chatTemplatesSummary", {
                                  count: chatTemplates?.length ?? 0,
                                })
                              : t("characters.edit.chatTemplatesEmptyDesc")}
                          </p>
                        </div>
                        <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 group-hover:text-fg/50" />
                      </button>

                      <button
                        type="button"
                        onClick={() =>
                          navigate(`/settings/customization/chat?characterId=${characterId}`)
                        }
                        className={cn(
                          "group flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left",
                          "border-fg/10 bg-surface-el/20",
                          interactive.transition.fast,
                          "hover:bg-surface-el/30",
                        )}
                      >
                        <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                          <MessageSquare className="h-4 w-4 text-info" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="text-sm font-medium text-fg">{t("characters.edit.chatAppearanceTitle")}</div>
                          <p className="mt-0.5 text-xs text-fg/50">
                            {t("characters.edit.chatAppearanceDesc")}
                          </p>
                        </div>
                        <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 group-hover:text-fg/50" />
                      </button>

                      <motion.button
                        onClick={() => setExportMenuOpen(true)}
                        disabled={exporting}
                        whileTap={{ scale: exporting ? 1 : 0.98 }}
                        className="flex w-full items-center justify-center gap-2 rounded-xl border border-info/40 bg-info/20 px-4 py-3.5 text-sm font-semibold text-info transition hover:bg-info/30 disabled:opacity-50"
                      >
                        {exporting ? (
                          <>
                            <Loader2 className="h-4 w-4 animate-spin" />
                            {t("characters.edit.exporting")}
                          </>
                        ) : (
                          <>
                            <Download className="h-4 w-4" />
                            {t("characters.edit.exportCharacter")}
                          </>
                        )}
                      </motion.button>
                    </div>
                  </div>
                </div>

                <div className="space-y-6">
                  <section className={spacing.field}>
                    <div className="flex items-center gap-2">
                      <div className="rounded-lg border border-fg/10 bg-fg/5 p-1.5">
                        <Info className="h-4 w-4 text-fg/60" />
                      </div>
                      <h3 className="text-sm font-semibold text-fg">{t("characters.edit.descriptionTitle")}</h3>
                    </div>
                    <textarea
                      value={description}
                      onChange={(e) => setFields({ description: e.target.value })}
                      rows={5}
                      placeholder={t("characters.edit.descriptionPlaceholder")}
                      className={cn(
                        "w-full resize-none border bg-surface-el/20 px-4 py-3.5 text-sm leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl",
                        radius.md,
                        interactive.transition.default,
                        "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                        description.trim() ? "border-fg/20" : "border-fg/10",
                      )}
                    />
                    <p className="text-xs text-fg/50">
                      {t("characters.edit.descriptionHint")}
                    </p>
                  </section>

                  <section className={spacing.field}>
                    <div className="space-y-3">
                      <div className="flex items-center gap-2">
                        <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                          <Sparkles className="h-4 w-4 text-accent" />
                        </div>
                        <h3 className="text-sm font-semibold text-fg">{t("characters.edit.definitionTitle")}</h3>
                      </div>
                      <textarea
                        value={definition}
                        onChange={(e) => setFields({ definition: e.target.value })}
                        rows={18}
                        placeholder={t("characters.edit.definitionPlaceholder")}
                        className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                      />
                      <div className="flex justify-between text-[11px] text-fg/50">
                        <span>{t("characters.edit.definitionDetailHint")}</span>
                        <span>{t("characters.edit.wordsCount", { count: wordCount(definition) })}</span>
                      </div>
                      <div className="rounded-xl border border-warning/30 bg-warning/10 px-3.5 py-3">
                        <div className="text-[11px] font-medium text-warning">
                          {t("characters.edit.availablePlaceholders")}
                        </div>
                        <div className="mt-2 space-y-1 text-xs text-fg/60">
                          <div>
                            <code className="text-accent">{"{{char}}"}</code> {t("characters.edit.placeholderChar")}
                          </div>
                          <div>
                            <code className="text-accent">{"{{user}}"}</code> {t("characters.edit.placeholderUser")}
                          </div>
                          <div>
                            <code className="text-accent">{"{{persona}}"}</code> {t("characters.edit.placeholderPersona")}
                          </div>
                        </div>
                      </div>
                    </div>
                  </section>

                  <section className={spacing.field}>
                    <div className="space-y-3">
                      <div className="flex items-center gap-2">
                        <div className="rounded-lg border border-fg/10 bg-fg/5 p-1.5">
                          <Image className="h-4 w-4 text-fg/60" />
                        </div>
                        <h3 className="text-sm font-semibold text-fg">{t("characters.edit.designReferencesTitle")}</h3>
                      </div>
                      <DesignReferenceEditor
                        designDescription={designDescription}
                        onDesignDescriptionChange={(value) =>
                          setFields({ designDescription: value })
                        }
                        referenceImages={designReferenceImageIds}
                        onReferenceImagesChange={(value) =>
                          setFields({ designReferenceImageIds: value })
                        }
                        subjectName={name}
                        subjectDescription={definition || description}
                        avatarImage={avatarPath}
                        showHeader={false}
                        description={t("characters.edit.designReferencesEditorHint")}
                      />
                      <LoraSelector
                        loraName={loraName}
                        loraStrength={loraStrength}
                        onChange={(name, strength) =>
                          setFields({ loraName: name, loraStrength: strength })
                        }
                      />
                    </div>
                  </section>
                </div>
              </div>

              {/* Starting Scenes Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <BookOpen className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">
                    {mode === "companion"
                      ? t("characters.edit.openingContextTitle")
                      : t("characters.edit.startingScenesTitle")}
                  </h3>
                  {scenes.length > 0 && (
                    <span className="ml-auto rounded-full border border-fg/10 bg-fg/5 px-2 py-0.5 text-xs text-fg/70">
                      {scenes.length}
                    </span>
                  )}
                </div>

                {/* Existing Scenes */}
                <AnimatePresence mode="popLayout">
                  {scenes.length > 0 && (
                    <motion.div layout className="space-y-2">
                      {scenes.map((scene, index) => {
                        const isDefault = defaultSceneId === scene.id;
                        const isExpanded = expandedSceneId === scene.id;

                        return (
                          <motion.div
                            key={scene.id}
                            layout
                            initial={{ opacity: 0, scale: 0.95 }}
                            animate={{ opacity: 1, scale: 1 }}
                            exit={{ opacity: 0, scale: 0.9, x: -20 }}
                            transition={{ duration: 0.15 }}
                            className={`overflow-hidden rounded-xl border ${
                              isDefault ? "border-accent/40 bg-accent/10" : "border-fg/15 bg-fg/8"
                            }`}
                          >
                            {/* Scene Header - clickable to expand/collapse */}
                            <button
                              onClick={() => setExpandedSceneId(isExpanded ? null : scene.id)}
                              className={`flex w-full items-center gap-2 border-b px-3.5 py-2.5 text-left ${
                                isDefault ? "border-accent/30 bg-accent/15" : "border-fg/15 bg-fg/8"
                              }`}
                            >
                              {/* Scene number badge */}
                              <div
                                className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-lg border text-xs font-medium ${
                                  isDefault
                                    ? "border-accent/40 bg-accent/20 text-accent/80"
                                    : "border-fg/10 bg-fg/5 text-fg/60"
                                }`}
                              >
                                {index + 1}
                              </div>

                              {/* Default badge */}
                              {isDefault && (
                                <div className="flex items-center gap-1 rounded-full border border-accent/40 bg-accent/20 px-2 py-0.5">
                                  <div className="h-1.5 w-1.5 rounded-full bg-accent" />
                                  <span className="text-[10px] font-medium text-accent/80">
                                    {t("characters.edit.sceneDefaultBadge")}
                                  </span>
                                </div>
                              )}

                              {/* Direction indicator */}
                              {scene.direction && (
                                <div
                                  className="flex items-center gap-1 rounded-full border border-fg/10 bg-fg/5 px-1.5 py-0.5"
                                  title={t("characters.edit.hasSceneDirection")}
                                >
                                  <EyeOff className="h-3 w-3 text-fg/40" />
                                </div>
                              )}

                              {/* Preview text when collapsed */}
                              {!isExpanded && (
                                <span className="flex-1 truncate text-sm text-fg/50">
                                  {scene.content.slice(0, 50)}
                                  {scene.content.length > 50 ? "..." : ""}
                                </span>
                              )}

                              {/* Expand indicator */}
                              <ChevronDown
                                className={cn(
                                  "h-4 w-4 text-fg/40 transition-transform ml-auto",
                                  isExpanded && "rotate-180",
                                )}
                              />
                            </button>

                            {/* Scene Content - collapsible */}
                            <AnimatePresence>
                              {isExpanded && (
                                <motion.div
                                  initial={{ height: 0, opacity: 0 }}
                                  animate={{ height: "auto", opacity: 1 }}
                                  exit={{ height: 0, opacity: 0 }}
                                  transition={{ duration: 0.2 }}
                                  className="overflow-hidden"
                                >
                                  <div className="p-3.5">
                                    <div className="space-y-3">
                                      <p className="text-sm leading-relaxed text-fg/90">
                                        {scene.content}
                                      </p>

                                      {/* Scene Direction (if set) */}
                                      {scene.direction && (
                                        <div className="pt-2 border-t border-fg/5">
                                          <p className="text-[10px] font-medium text-fg/40 mb-1">
                                            {t("characters.edit.sceneDirectionLabel")}
                                          </p>
                                          <p className="text-xs leading-relaxed text-fg/50 italic">
                                            {scene.direction}
                                          </p>
                                        </div>
                                      )}

                                      {scene.backgroundImagePath && (
                                        <SceneBackgroundCard
                                          path={scene.backgroundImagePath}
                                          label={t("characters.edit.sceneBackgroundLabel")}
                                          compact
                                        />
                                      )}

                                      {/* Actions when expanded */}
                                      <div className="flex items-center gap-2 pt-2 border-t border-fg/5">
                                        {!isDefault && (
                                          <button
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              setFields({ defaultSceneId: scene.id });
                                            }}
                                            className="rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-xs font-medium text-fg/60 transition active:scale-95 active:bg-fg/10"
                                          >
                                            {t("characters.edit.setAsDefault")}
                                          </button>
                                        )}
                                        <button
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            setNewSceneEditorOpen(false);
                                            startEditingScene(scene);
                                          }}
                                          className="rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/60 transition active:scale-95 active:bg-fg/10"
                                          aria-label={t("characters.edit.editSceneAria", { number: index + 1 })}
                                        >
                                          <Edit2 className="h-3.5 w-3.5" />
                                        </button>
                                        <button
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            deleteScene(scene.id);
                                          }}
                                          className="rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/50 transition active:bg-danger/10 active:text-danger"
                                          aria-label={t("characters.edit.deleteSceneAria", { number: index + 1 })}
                                        >
                                          <X className="h-3.5 w-3.5" />
                                        </button>
                                      </div>
                                    </div>
                                  </div>
                                </motion.div>
                              )}
                            </AnimatePresence>
                          </motion.div>
                        );
                      })}
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Add New Scene */}
                <motion.div layout className="space-y-2">
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3">
                    <div className="text-sm font-medium text-fg">
                      {mode === "companion"
                        ? t("characters.edit.newOpeningContextTitle")
                        : t("characters.edit.newStartingSceneTitle")}
                    </div>
                    <p className="mt-1 text-xs text-fg/50">
                      {mode === "companion"
                        ? t("characters.edit.newOpeningContextDesc")
                        : t("characters.edit.newStartingSceneDesc")}
                    </p>
                    <div className="mt-3 flex items-center gap-2">
                      <motion.button
                        onClick={() => setNewSceneEditorOpen(true)}
                        whileTap={{ scale: 0.97 }}
                        className="flex items-center gap-2 rounded-xl border border-accent/50 bg-accent/20 px-3.5 py-2 text-sm font-medium text-accent transition active:bg-accent/30"
                      >
                        <Plus className="h-4 w-4" />
                        {t("characters.edit.createScene")}
                      </motion.button>
                      {newSceneContent.trim() && (
                        <button
                          type="button"
                          onClick={() => setNewSceneEditorOpen(true)}
                          className="text-xs text-fg/50 transition hover:text-fg/70"
                        >
                          {t("characters.edit.continueDraft")}
                        </button>
                      )}
                    </div>
                  </div>
                </motion.div>

                <p className="text-xs text-fg/50">
                  {mode === "companion"
                    ? t("characters.edit.companionScenesOptionalHint")
                    : t("characters.edit.multipleScenesHint")}
                </p>
              </div>
            </>
          )}

          {/* Settings Tab: Model & Memory */}
          {activeTab === "settings" && (
            <>
              <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
                <div className="space-y-4">
                  {/* Voice Selection */}
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                        <Volume2 className="h-4 w-4 text-accent/80" />
                      </div>
                      <h3 className="text-sm font-semibold text-fg">{t("characters.edit.voiceTitle")}</h3>
                      <span className="ml-auto text-xs text-fg/40">{t("characters.edit.optionalSuffix")}</span>
                    </div>

                    {loadingVoices ? (
                      <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                        <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                        <span className="text-sm text-fg/50">{t("characters.edit.loadingVoices")}</span>
                      </div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => setShowVoiceMenu(true)}
                        className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                      >
                        <div className="flex items-center gap-2">
                          <Volume2 className="h-5 w-5 text-fg/40" />
                          <span
                            className={`text-sm ${voiceSelectionValue ? "text-fg" : "text-fg/50"}`}
                          >
                            {voiceSelectionValue
                              ? (() => {
                                  if (voiceConfig?.source === "user") {
                                    const v = userVoices.find(
                                      (uv) => uv.id === voiceConfig.userVoiceId,
                                    );
                                    return v?.name || t("characters.edit.customVoiceFallback");
                                  }
                                  if (voiceConfig?.source === "provider") {
                                    const pv = providerVoices[voiceConfig.providerId || ""]?.find(
                                      (pv) => pv.voiceId === voiceConfig.voiceId,
                                    );
                                    return pv?.name || t("characters.edit.providerVoiceFallback");
                                  }
                                  return t("characters.edit.selectedVoiceFallback");
                                })()
                              : t("characters.edit.noVoiceAssigned")}
                          </span>
                        </div>
                        <ChevronDown className="h-4 w-4 text-fg/40" />
                      </button>
                    )}

                    {voiceError && <p className="text-xs font-medium text-danger">{voiceError}</p>}
                    {!loadingVoices && audioProviders.length === 0 && userVoices.length === 0 && (
                      <p className="text-xs text-fg/40">{t("characters.edit.addVoicesHint")}</p>
                    )}
                    <p className="text-xs text-fg/50">
                      {t("characters.edit.voiceAssignHint")}
                    </p>
                    <div
                      className={cn(
                        "flex items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3",
                        !voiceConfig && "opacity-50",
                      )}
                    >
                      <div>
                        <p className="text-sm font-medium text-fg">{t("characters.edit.autoplayLabel")}</p>
                        <p className="mt-1 text-xs text-fg/50">
                          {voiceConfig
                            ? t("characters.edit.autoplayOn")
                            : t("characters.edit.autoplayOff")}
                        </p>
                      </div>
                      <Switch
                        id="character-voice-autoplay"
                        checked={voiceAutoplay}
                        onChange={(next) => setFields({ voiceAutoplay: next })}
                        disabled={!voiceConfig}
                      />
                    </div>
                  </div>

                  {/* Memory Mode */}
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                        <Layers className="h-4 w-4 text-warning" />
                      </div>
                      <h3 className="text-sm font-semibold text-fg">{t("characters.edit.memoryModeTitle")}</h3>
                      {!dynamicMemoryEnabled && (
                        <span className="ml-auto text-xs text-fg/40">
                          {t("characters.edit.enableDynamicMemoryHint")}
                        </span>
                      )}
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <button
                        type="button"
                        onClick={() => setFields({ memoryType: "manual" })}
                        className={`rounded-xl border px-3.5 py-3 text-left transition ${
                          memoryType === "manual"
                            ? "border-accent/40 bg-accent/15 shadow-[0_0_0_1px_rgba(16,185,129,0.25)]"
                            : "border-fg/15 bg-surface-el/20 hover:border-fg/20 hover:bg-surface-el/30"
                        }`}
                      >
                        <p
                          className={`text-sm font-semibold ${memoryType === "manual" ? "text-fg" : "text-fg/70"}`}
                        >
                          {t("characters.edit.memoryManualTitle")}
                        </p>
                        <p className="mt-1 text-xs text-fg/50">
                          {t("characters.edit.memoryManualDesc")}
                        </p>
                      </button>
                      <button
                        type="button"
                        disabled={!dynamicMemoryEnabled}
                        onClick={() => dynamicMemoryEnabled && setFields({ memoryType: "dynamic" })}
                        className={`rounded-xl border px-3.5 py-3 text-left transition ${
                          memoryType === "dynamic" && dynamicMemoryEnabled
                            ? "border-info/50 bg-info/20 shadow-[0_0_0_1px_rgba(96,165,250,0.3)]"
                            : "border-fg/15 bg-surface-el/15"
                        } ${!dynamicMemoryEnabled ? "cursor-not-allowed opacity-50" : "hover:border-fg/20 hover:bg-surface-el/25"}`}
                      >
                        <p
                          className={`text-sm font-semibold ${memoryType === "dynamic" && dynamicMemoryEnabled ? "text-fg" : "text-fg/70"}`}
                        >
                          {t("characters.edit.memoryDynamicTitle")}
                        </p>
                        <p className="mt-1 text-xs text-fg/50">
                          {t("characters.edit.memoryDynamicDesc")}
                        </p>
                      </button>
                    </div>
                    <p className="text-xs text-fg/50">
                      {t("characters.edit.memoryModeHint")}
                    </p>
                  </div>
                </div>

                <div className="space-y-4">
                  {/* Default Model */}
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
                        <Cpu className="h-4 w-4 text-secondary" />
                      </div>
                      <h3 className="text-sm font-semibold text-fg">{t("characters.edit.defaultModelTitle")}</h3>
                      <span className="ml-auto text-xs text-fg/40">{t("characters.edit.optionalSuffix")}</span>
                    </div>

                    {loadingModels ? (
                      <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                        <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                        <span className="text-sm text-fg/50">{t("characters.edit.loadingModels")}</span>
                      </div>
                    ) : models.length > 0 ? (
                      <button
                        type="button"
                        onClick={() => setShowModelMenu(true)}
                        className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                      >
                        <div className="flex items-center gap-2">
                          {selectedModelId ? (
                            getProviderIcon(
                              models.find((m) => m.id === selectedModelId)?.providerId || "",
                            )
                          ) : (
                            <Cpu className="h-5 w-5 text-fg/40" />
                          )}
                          <span className={`text-sm ${selectedModelId ? "text-fg" : "text-fg/50"}`}>
                            {selectedModelId
                              ? models.find((m) => m.id === selectedModelId)?.displayName ||
                                t("characters.edit.selectedModelFallback")
                              : t("characters.edit.useGlobalDefaultModel")}
                          </span>
                        </div>
                        <ChevronDown className="h-4 w-4 text-fg/40" />
                      </button>
                    ) : (
                      <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                        <p className="text-sm text-fg/50">{t("characters.edit.noModelsAvailable")}</p>
                      </div>
                    )}
                    <p className="text-xs text-fg/50">
                      {t("characters.edit.defaultModelHint")}
                    </p>
                  </div>

                  <ActiveLorebooksSelector
                    selectedIds={activeLorebookIds}
                    onChange={(ids) => setFields({ activeLorebookIds: ids })}
                    disabled={saving}
                  />
                </div>
              </div>

              {/* Prompt Templates */}
              <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
                {/* System Prompt */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
                      <MessageSquare className="h-4 w-4 text-secondary" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">{t("characters.edit.systemPromptTitle")}</h3>
                    <span className="ml-auto text-xs text-fg/40">{t("characters.edit.optionalSuffix")}</span>
                  </div>

                  {loadingTemplates ? (
                    <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                      <span className="text-sm text-fg/50">{t("characters.edit.loadingTemplates")}</span>
                    </div>
                  ) : promptTemplates.length > 0 ? (
                    <select
                      value={systemPromptTemplateId || ""}
                      onChange={(e) =>
                        setFields({ systemPromptTemplateId: e.target.value || null })
                      }
                      className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                    >
                      <option value="">{t("characters.edit.useDefaultSystemPrompt")}</option>
                      {directPromptTemplates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <p className="text-sm text-fg/50">{t("characters.edit.usingAppDefault")}</p>
                      <p className="mt-1 text-xs text-fg/40">{t("characters.edit.noDirectTemplatesHint")}</p>
                    </div>
                  )}
                  <p className="text-xs text-fg/50">{t("characters.edit.systemPromptOverrideHint")}</p>
                </div>

                {/* Companion Prompt */}
                <div
                  className={cn(
                    "space-y-3",
                    mode !== "companion" && "pointer-events-none opacity-50",
                  )}
                  aria-disabled={mode !== "companion"}
                >
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-danger/30 bg-danger/10 p-1.5">
                      <Heart className="h-4 w-4 text-danger" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">{t("characters.edit.companionPromptTitle")}</h3>
                    <span className="ml-auto text-xs text-fg/40">
                      {mode === "companion"
                        ? t("characters.edit.optionalSuffix")
                        : t("characters.edit.companionModeRequiredHint")}
                    </span>
                  </div>

                  {loadingTemplates ? (
                    <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                      <span className="text-sm text-fg/50">{t("characters.edit.loadingTemplates")}</span>
                    </div>
                  ) : promptTemplates.length > 0 ? (
                    <select
                      value={companionPromptTemplateId || ""}
                      disabled={mode !== "companion"}
                      onChange={(e) =>
                        setFields({ companionPromptTemplateId: e.target.value || null })
                      }
                      className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                    >
                      <option value="">{t("characters.edit.useDefaultCompanionPrompt")}</option>
                      {companionPromptTemplates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <p className="text-sm text-fg/50">{t("characters.edit.usingAppDefault")}</p>
                      <p className="mt-1 text-xs text-fg/40">{t("characters.edit.noCompanionTemplatesHint")}</p>
                    </div>
                  )}
                  <p className="text-xs text-fg/50">{t("characters.edit.companionPromptStoredHint")}</p>
                </div>

                {/* Group Chat Prompt (Conversation) */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                      <Users className="h-4 w-4 text-info" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">{t("characters.edit.groupChatPromptTitle")}</h3>
                    <span className="ml-auto text-xs text-fg/40">{t("characters.edit.conversationSuffix")}</span>
                  </div>

                  {loadingTemplates ? (
                    <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                      <span className="text-sm text-fg/50">{t("characters.edit.loadingTemplates")}</span>
                    </div>
                  ) : groupChatTemplates.length > 0 ? (
                    <select
                      value={groupChatPromptTemplateId || ""}
                      onChange={(e) =>
                        setFields({ groupChatPromptTemplateId: e.target.value || null })
                      }
                      className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                    >
                      <option value="">{t("characters.edit.useDefaultGroupConversationPrompt")}</option>
                      {groupChatTemplates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <p className="text-sm text-fg/50">{t("characters.edit.usingAppDefault")}</p>
                      <p className="mt-1 text-xs text-fg/40">
                        {t("characters.edit.noGroupConversationTemplatesHint")}
                      </p>
                    </div>
                  )}
                  <p className="text-xs text-fg/50">
                    {t("characters.edit.groupConversationOverrideHint")}
                  </p>
                </div>

                {/* Group Chat Prompt (Roleplay) */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                      <Drama className="h-4 w-4 text-warning" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">{t("characters.edit.groupChatPromptTitle")}</h3>
                    <span className="ml-auto text-xs text-fg/40">{t("characters.edit.roleplaySuffix")}</span>
                  </div>

                  {loadingTemplates ? (
                    <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                      <span className="text-sm text-fg/50">{t("characters.edit.loadingTemplates")}</span>
                    </div>
                  ) : groupChatRoleplayTemplates.length > 0 ? (
                    <select
                      value={groupChatRoleplayPromptTemplateId || ""}
                      onChange={(e) =>
                        setFields({
                          groupChatRoleplayPromptTemplateId: e.target.value || null,
                        })
                      }
                      className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                    >
                      <option value="">{t("characters.edit.useDefaultGroupRoleplayPrompt")}</option>
                      {groupChatRoleplayTemplates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <p className="text-sm text-fg/50">{t("characters.edit.usingAppDefault")}</p>
                      <p className="mt-1 text-xs text-fg/40">
                        {t("characters.edit.noGroupRoleplayTemplatesHint")}
                      </p>
                    </div>
                  )}
                  <p className="text-xs text-fg/50">
                    {t("characters.edit.groupRoleplayOverrideHint")}
                  </p>
                </div>
              </div>
            </>
          )}
        </motion.div>
      </main>

      <CharacterExportMenu
        isOpen={exportMenuOpen}
        onClose={() => setExportMenuOpen(false)}
        onSelect={handleExportFormat}
        exporting={exporting}
      />

      {/* Bottom Tab Bar */}
      <div
        className={cn(
          "fixed bottom-0 left-0 right-0 border-t px-3 pb-[calc(env(safe-area-inset-bottom)+12px)] pt-3",
          colors.glass.strong,
        )}
      >
        <div
          role="tablist"
          aria-label={t("characters.edit.tabsAria")}
          className={cn(
            radius.lg,
            "grid gap-2 p-1",
            tabItems.length === 3 ? "grid-cols-3" : "grid-cols-2",
            colors.surface.elevated,
          )}
        >
          {tabItems.map(({ id, icon: Icon, label, disabled: tabDisabled, hint }) => (
            <button
              key={id}
              type="button"
              onClick={() => !tabDisabled && setActiveTab(id)}
              disabled={tabDisabled}
              title={hint}
              role="tab"
              id={id === "character" ? characterTabId : id === "soul" ? soulTabId : settingsTabId}
              aria-selected={activeTab === id}
              aria-controls={tabPanelId}
              aria-disabled={tabDisabled}
              className={cn(
                radius.md,
                "px-3 py-2.5 text-sm font-semibold transition flex items-center justify-center gap-2",
                interactive.active.scale,
                tabDisabled
                  ? "cursor-not-allowed text-fg/30"
                  : activeTab === id
                    ? "bg-fg/10 text-fg"
                    : cn(colors.text.tertiary, "hover:text-fg"),
              )}
            >
              <Icon size={16} className="block" />
              <span className="pt-1">{label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Edit/New Scene Fullscreen Panel */}
      <AnimatePresence>
        {(editingSceneId !== null || newSceneEditorOpen) && (
          <motion.div
            className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h,0px)] z-50 flex flex-col bg-surface"
            style={{ paddingTop: "env(safe-area-inset-top)" }}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 20 }}
            transition={{ duration: 0.2 }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
              <div className="text-base font-semibold text-fg">
                {editingSceneId !== null
                  ? t("characters.edit.editSceneTitle")
                  : t("characters.edit.newSceneTitle")}
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={editingSceneId !== null ? cancelEditingScene : closeNewSceneEditor}
                  className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                >
                  {t("characters.edit.close")}
                </button>
                <button
                  type="button"
                  onClick={editingSceneId !== null ? saveEditedScene : saveNewScene}
                  disabled={
                    editingSceneId !== null ? !editingSceneContent.trim() : !newSceneContent.trim()
                  }
                  className={cn(
                    "rounded-full px-3 py-1.5 text-xs font-semibold text-fg transition",
                    "bg-linear-to-r from-accent to-accent/80",
                    "hover:from-accent/80 hover:to-accent/60",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                  )}
                >
                  {editingSceneId !== null
                    ? t("common.buttons.save")
                    : t("common.buttons.add")}
                </button>
              </div>
            </div>

            <div className="flex-1 overflow-y-auto px-4 pb-6 pt-4">
              <input
                ref={sceneBackgroundInputRef}
                type="file"
                accept="image/*"
                className="hidden"
                onChange={(event) => {
                  void handleSceneBackgroundUpload(event);
                }}
              />
              <div className="space-y-6">
                <div className="space-y-2">
                  <div className="text-sm font-medium text-fg/80">{t("characters.edit.sceneLabel")}</div>
                  <textarea
                    ref={sceneContentRef}
                    value={editingSceneId !== null ? editingSceneContent : newSceneContent}
                    onChange={(e) =>
                      setFields(
                        editingSceneId !== null
                          ? { editingSceneContent: e.target.value }
                          : { newSceneContent: e.target.value },
                      )
                    }
                    rows={14}
                    className="min-h-[40vh] w-full resize-none rounded-2xl border border-fg/10 bg-surface-el/40 px-4 py-4 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/20 focus:outline-none"
                    placeholder={t("characters.edit.sceneContentPlaceholder")}
                  />
                  <div className="flex items-center justify-between text-[11px] text-fg/40">
                    <span>
                      {t("characters.edit.wordsCount", {
                        count: wordCount(
                          editingSceneId !== null ? editingSceneContent : newSceneContent,
                        ),
                      })}
                    </span>
                    <span>
                      {t("characters.edit.usePrefix")}{" "}
                      <code className="text-accent/80">{"{{char}}"}</code>,{" "}
                      <code className="text-accent/80">{"{{user}}"}</code>
                    </span>
                  </div>
                  <input
                    ref={sceneInlineImageInputRef}
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={(event) => {
                      void handleInlineImageUpload(event);
                    }}
                  />
                  <InlineImageToolbar
                    addLabel={t("sceneImage.add")}
                    uploadLabel={t("sceneImage.upload")}
                    libraryLabel={t("sceneImage.fromLibrary")}
                    hint={t("sceneImage.hint")}
                    onUpload={() => sceneInlineImageInputRef.current?.click()}
                    onLibrary={handleChooseInlineImageFromLibrary}
                  />
                </div>

                <div className="space-y-2">
                  <div className="flex items-center gap-1.5 text-sm font-medium text-fg/80">
                    <EyeOff className="h-4 w-4 text-fg/50" />
                    {t("characters.edit.sceneDirectionLabel")}
                  </div>
                  <textarea
                    value={editingSceneId !== null ? editingSceneDirection : newSceneDirection}
                    onChange={(e) =>
                      setFields(
                        editingSceneId !== null
                          ? { editingSceneDirection: e.target.value }
                          : { newSceneDirection: e.target.value },
                      )
                    }
                    rows={6}
                    className="min-h-[18vh] w-full resize-none rounded-2xl border border-fg/10 bg-surface-el/35 px-4 py-3 text-sm leading-relaxed text-fg placeholder-fg/30 transition focus:border-fg/20 focus:outline-none"
                    placeholder={t("characters.edit.sceneDirectionPlaceholder")}
                  />
                  <p className="text-[11px] text-fg/40">
                    {t("characters.edit.sceneDirectionHint")}
                  </p>
                </div>

                <div className="space-y-3">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="flex items-center gap-1.5 text-sm font-medium text-fg/80">
                        <Image className="h-4 w-4 text-fg/50" />
                        {t("characters.edit.sceneBackgroundTitle")}
                      </div>
                      <p className="mt-1 text-[11px] text-fg/40">
                        {t("characters.edit.sceneBackgroundHint")}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={() =>
                          handleChooseSceneBackgroundFromLibrary(
                            editingSceneId !== null ? "edit" : "new",
                          )
                        }
                        className="flex items-center gap-2 rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/60 transition hover:bg-fg/10 hover:text-fg"
                      >
                        <FolderOpen className="h-3.5 w-3.5" />
                        {t("characters.edit.library")}
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          (editingSceneId !== null
                            ? editingSceneBackgroundImagePath
                            : newSceneBackgroundImagePath)
                            ? setFields(
                                editingSceneId !== null
                                  ? { editingSceneBackgroundImagePath: "" }
                                  : { newSceneBackgroundImagePath: "" },
                              )
                            : sceneBackgroundInputRef.current?.click()
                        }
                        className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                      >
                        {(editingSceneId !== null
                          ? editingSceneBackgroundImagePath
                          : newSceneBackgroundImagePath)
                          ? t("common.buttons.remove")
                          : t("characters.edit.upload")}
                      </button>
                    </div>
                  </div>

                  {(editingSceneId !== null
                    ? editingSceneBackgroundImagePath
                    : newSceneBackgroundImagePath) ? (
                    <div>
                      <SceneBackgroundCard
                        path={
                          editingSceneId !== null
                            ? editingSceneBackgroundImagePath
                            : newSceneBackgroundImagePath
                        }
                        label={t("characters.edit.sceneBackgroundPreviewLabel")}
                      />
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Background Upload Choice Menu */}
      <BottomMenu
        isOpen={showBackgroundChoiceMenu}
        onClose={() => {
          setShowBackgroundChoiceMenu(false);
          setPendingBackgroundSrc(null);
        }}
        title=""
      >
        <div className="space-y-4 px-1 pb-2">
          <div className="text-center">
            <h3 className="text-lg font-semibold text-fg">{t("characters.edit.backgroundImageMenuTitle")}</h3>
            <p className="text-sm text-fg/50">{t("characters.edit.backgroundImageMenuDesc")}</p>
          </div>

          <MenuSection>
            <MenuButtonGroup className="space-y-2">
              <MenuButton
                icon={Upload}
                title={t("characters.edit.quickUploadTitle")}
                description={t("characters.edit.quickUploadDesc")}
                color="from-emerald-500 to-emerald-600"
                onClick={() => {
                  if (pendingBackgroundSrc) {
                    setFields({ backgroundImagePath: pendingBackgroundSrc });
                  }
                  setShowBackgroundChoiceMenu(false);
                  setPendingBackgroundSrc(null);
                }}
              />
              <MenuButton
                icon={Crop}
                title={t("characters.edit.positionCropTitle")}
                description={t("characters.edit.positionCropDesc")}
                color="from-blue-500 to-blue-600"
                onClick={() => {
                  setShowBackgroundChoiceMenu(false);
                  setShowBackgroundPositionModal(true);
                }}
              />
            </MenuButtonGroup>
          </MenuSection>
        </div>
      </BottomMenu>

      {/* Background Position Modal */}
      {pendingBackgroundSrc && (
        <BackgroundPositionModal
          isOpen={showBackgroundPositionModal}
          onClose={() => {
            setShowBackgroundPositionModal(false);
            setPendingBackgroundSrc(null);
          }}
          imageSrc={pendingBackgroundSrc}
          onConfirm={(croppedDataUrl) => {
            setFields({ backgroundImagePath: croppedDataUrl });
            setPendingBackgroundSrc(null);
          }}
        />
      )}

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("characters.edit.selectModelTitle")}
        models={models}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        searchPlaceholder={t("characters.edit.searchModelsPlaceholder")}
        onSelectModel={(modelId) => {
          setFields({ selectedModelId: modelId });
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("characters.edit.useGlobalDefaultModel"),
          icon: Cpu,
          selected: !selectedModelId,
          onClick: () => {
            setFields({ selectedModelId: null });
            setShowModelMenu(false);
          },
        }}
      />

      {/* Voice Selection BottomMenu */}
      <BottomMenu
        isOpen={showVoiceMenu}
        onClose={() => {
          setShowVoiceMenu(false);
          setVoiceSearchQuery("");
        }}
        title={t("characters.edit.selectVoiceTitle")}
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={voiceSearchQuery}
              onChange={(e) => setVoiceSearchQuery(e.target.value)}
              placeholder={t("characters.edit.searchVoicesPlaceholder")}
              className="w-full rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none"
            />
            <svg
              className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>
          <div className="space-y-2 max-h-[50vh] overflow-y-auto">
            <button
              onClick={() => {
                setFields({ voiceConfig: null });
                setShowVoiceMenu(false);
                setVoiceSearchQuery("");
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                !voiceSelectionValue
                  ? "border-accent/40 bg-accent/10"
                  : "border-fg/10 bg-fg/5 hover:bg-fg/10",
              )}
            >
              <Volume2 className="h-5 w-5 text-fg/40" />
              <span className="text-sm text-fg">{t("characters.edit.noVoiceAssigned")}</span>
              {!voiceSelectionValue && <Check className="h-4 w-4 ml-auto text-accent" />}
            </button>

            {/* User Voices */}
            {userVoices.length > 0 && (
              <MenuSection label={t("characters.edit.myVoices")}>
                {userVoices
                  .filter((v) => {
                    if (!voiceSearchQuery) return true;
                    return v.name.toLowerCase().includes(voiceSearchQuery.toLowerCase());
                  })
                  .map((voice) => {
                    const value = buildUserVoiceValue(voice.id);
                    const isSelected = voiceSelectionValue === value;
                    const providerLabel =
                      audioProviders.find((p) => p.id === voice.providerId)?.label ??
                      t("characters.edit.providerFallback");
                    return (
                      <button
                        key={voice.id}
                        onClick={() => {
                          setFields({
                            voiceConfig: {
                              source: "user",
                              userVoiceId: voice.id,
                              providerId: voice.providerId,
                              modelId: voice.modelId,
                              voiceName: voice.name,
                            },
                          });
                          setShowVoiceMenu(false);
                          setVoiceSearchQuery("");
                        }}
                        className={cn(
                          "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                          isSelected
                            ? "border-accent/40 bg-accent/10"
                            : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                        )}
                      >
                        <User className="h-5 w-5 text-fg/40" />
                        <div className="flex-1 min-w-0">
                          <span className="block truncate text-sm text-fg">{voice.name}</span>
                          <span className="block truncate text-xs text-fg/40">{providerLabel}</span>
                        </div>
                        {isSelected && <Check className="h-4 w-4 shrink-0 text-accent" />}
                      </button>
                    );
                  })}
              </MenuSection>
            )}

            {/* Provider Voices */}
            {audioProviders.map((provider) => {
              const voices = (providerVoices[provider.id] ?? []).filter((v) => {
                if (!voiceSearchQuery) return true;
                return v.name.toLowerCase().includes(voiceSearchQuery.toLowerCase());
              });
              if (voices.length === 0) return null;
              return (
                <MenuSection
                  key={provider.id}
                  label={t("characters.edit.providerVoicesLabel", { provider: provider.label })}
                >
                  {voices.map((voice) => {
                    const value = buildProviderVoiceValue(provider.id, voice.voiceId);
                    const isSelected = voiceSelectionValue === value;
                    return (
                      <button
                        key={`${provider.id}:${voice.voiceId}`}
                        onClick={() => {
                          setFields({
                            voiceConfig: {
                              source: "provider",
                              providerId: provider.id,
                              voiceId: voice.voiceId,
                              modelId:
                                provider.providerType === "kokoro"
                                  ? provider.kokoroVariant
                                  : undefined,
                              voiceName: voice.name,
                            },
                          });
                          setShowVoiceMenu(false);
                          setVoiceSearchQuery("");
                        }}
                        className={cn(
                          "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                          isSelected
                            ? "border-accent/40 bg-accent/10"
                            : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                        )}
                      >
                        <Volume2 className="h-5 w-5 text-fg/40" />
                        <span className="flex-1 truncate text-sm text-fg">{voice.name}</span>
                        {isSelected && <Check className="h-4 w-4 shrink-0 text-accent" />}
                      </button>
                    );
                  })}
                </MenuSection>
              );
            })}
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

function SceneBackgroundCard({
  path,
  label,
  compact = false,
}: {
  path: string;
  label: string;
  compact?: boolean;
}) {
  const imageData = useImageData(path) ?? path;

  return (
    <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/4">
      <img
        src={imageData}
        alt={label}
        className={cn("w-full object-cover", compact ? "h-24" : "h-32")}
      />
      <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/60">{label}</div>
    </div>
  );
}

function InlineImageToolbar({
  addLabel,
  uploadLabel,
  libraryLabel,
  hint,
  onUpload,
  onLibrary,
}: {
  addLabel: string;
  uploadLabel: string;
  libraryLabel: string;
  hint: string;
  onUpload: () => void;
  onLibrary: () => void;
}) {
  return (
    <div className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5">
      <div className="flex items-center gap-2">
        <Image className="h-3.5 w-3.5 text-fg/50" />
        <span className="text-xs font-medium text-fg/70">{addLabel}</span>
        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={onLibrary}
            className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[11px] font-medium text-fg/70 transition active:scale-95 active:bg-fg/10"
          >
            <FolderOpen className="h-3.5 w-3.5" />
            {libraryLabel}
          </button>
          <button
            type="button"
            onClick={onUpload}
            className="flex items-center gap-1.5 rounded-lg border border-accent/30 bg-accent/10 px-2.5 py-1.5 text-[11px] font-medium text-accent/90 transition active:scale-95 active:bg-accent/20"
          >
            <Upload className="h-3.5 w-3.5" />
            {uploadLabel}
          </button>
        </div>
      </div>
      <p className="mt-1.5 text-[10px] text-fg/40">{hint}</p>
    </div>
  );
}
