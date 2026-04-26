import {
  CSSProperties,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useParams, useSearchParams, useNavigate } from "react-router-dom";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import { ArrowLeftRight, ChevronDown, NotebookPen, User, X } from "lucide-react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AccessibilitySettings,
  Character,
  Model,
  Persona,
  Scene,
  StoredMessage,
} from "../../../core/storage/schemas";
import { createDefaultAccessibilitySettings } from "../../../core/storage/schemas";
import {
  abortAudioPreview,
  generateTtsForMessage,
  listAudioModels,
  listAudioProviders,
  listUserVoices,
  playAudioFromBase64,
  type AudioModel,
  type AudioProvider,
  type AudioProviderType,
  type TtsPreviewResponse,
  type UserVoice,
} from "../../../core/storage/audioProviders";
import { useChatLayoutContext } from "./ChatLayout";
import {
  createBranchedGroupSession,
  generateUserReply,
  getSession,
  getSessionMeta,
  listCharacters,
  listPersonas,
  readSettings,
  saveSession,
  SETTINGS_UPDATED_EVENT,
  SESSION_UPDATED_EVENT,
  updateSessionBackgroundImage,
} from "../../../core/storage";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { playAccessibilitySound } from "../../../core/utils/accessibilityAudio";
import { replacePlaceholders } from "../../../core/utils/placeholders";
import { splitThinkTags } from "../../../core/utils/thinkTags";
import { getPlatform } from "../../../core/utils/platform";
import {
  ChatHeader,
  ChatFooter,
  ChatMessage,
  MessageActionsBottomSheet,
  LoadingSpinner,
  EmptyState,
  ChatSettingsDrawer,
  AuthorNoteBottomMenu,
  InlineAuthorNoteBar,
  useAuthorNoteInlineEditor,
} from "./components";
import { BottomMenu, GuidedTour, MenuButton, useGuidedTour } from "../../components";
import { AvatarImage } from "../../components/AvatarImage";
import { useAvatar } from "../../hooks/useAvatar";
import { Image, RefreshCw, Sparkles, Check, PenLine, Lock } from "lucide-react";
import { radius, cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { PersonaSelector } from "../group-chats/components/settings";
import { sanitizeAssistantSceneDirective } from "./hooks/sceneImageProtocol";
import { useBeetrootRain } from "./components/BeetrootRain";
import { useBeetrootEasterEgg } from "./hooks/useBeetrootEasterEgg";
import { processBackgroundImage } from "../../../core/utils/image";
import { convertToImageRef } from "../../../core/storage/images";
import { useImageData } from "../../hooks/useImageData";
import { ImageLibraryPanel } from "../library/ImageLibraryPage";
import type { ImageLibraryItem } from "../../../core/storage/repo";
import {
  SCENE_PROMPT_APPROVAL_EVENT,
  type ScenePromptApprovalDetail,
} from "./hooks/useChatEnhancementsController";

const LONG_PRESS_DELAY = 450;
const SCROLL_THRESHOLD = 10; // pixels of movement to cancel long press
const AUTOLOAD_TOP_THRESHOLD_PX = 120;
const STICKY_BOTTOM_THRESHOLD_PX = 80;
const MAX_AUDIO_CACHE_ENTRIES = 50;
const MOBILE_KEYBOARD_THRESHOLD_PX = 120;

export function ChatConversationPage() {
  const { characterId } = useParams<{ characterId: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { t } = useI18n();
  const { shouldShow: showChatDetailTour, dismiss: dismissChatDetailTour } =
    useGuidedTour("chatDetail");
  const {
    shouldShow: showPostFirstMessageTour,
    dismiss: dismissPostFirstMessageTour,
    show: triggerPostFirstMessageTour,
  } = useGuidedTour("postFirstMessage");
  const sessionId = searchParams.get("sessionId") || undefined;
  const jumpToMessageId = searchParams.get("jumpToMessage");
  const {
    backgroundImageData,
    backgroundImageLoading,
    isBackgroundLight,
    theme,
    chatAppearance,
    chatController,
  } = useChatLayoutContext();

  const scrollContainerRef = useRef<HTMLElement | null>(null);
  const pressStartPosition = useRef<{ x: number; y: number } | null>(null);
  const [sessionForHeader, setSessionForHeader] = useState(chatController.session);
  const pendingScrollAdjustRef = useRef<{ prevScrollTop: number; prevScrollHeight: number } | null>(
    null,
  );
  const loadingOlderRef = useRef(false);
  const isAtBottomRef = useRef(true);
  const [isAtBottom, setIsAtBottom] = useState(true);

  const [showCharacterSelector, setShowCharacterSelector] = useState(false);
  const [showGroupCharacterSelector, setShowGroupCharacterSelector] = useState(false);
  const [availableCharacters, setAvailableCharacters] = useState<Character[]>([]);
  const [messageToBranch, setMessageToBranch] = useState<StoredMessage | null>(null);
  const [groupBranchSelectedIds, setGroupBranchSelectedIds] = useState<Set<string>>(new Set());
  const [groupBranchError, setGroupBranchError] = useState<string | null>(null);
  const [groupBranchCreating, setGroupBranchCreating] = useState(false);
  const [selectedImage, setSelectedImage] = useState<{ src: string; alt: string } | null>(null);
  const [selectedImagePromptExpanded, setSelectedImagePromptExpanded] = useState(false);
  const [supportsImageInput, setSupportsImageInput] = useState(false);
  const [keyboardInset, setKeyboardInset] = useState(0);
  const audioCacheRef = useRef<{
    providers: AudioProvider[] | null;
    userVoices: UserVoice[] | null;
    modelsByProviderType: Map<AudioProviderType, AudioModel[]>;
  }>({
    providers: null,
    userVoices: null,
    modelsByProviderType: new Map(),
  });
  const [accessibilitySettings, setAccessibilitySettings] = useState<AccessibilitySettings>(
    createDefaultAccessibilitySettings(),
  );
  const audioPreviewCacheRef = useRef<Map<string, TtsPreviewResponse>>(new Map());
  const [audioStatusByMessage, setAudioStatusByMessage] = useState<
    Record<string, "loading" | "playing">
  >({});
  const audioPlaybackRef = useRef<HTMLAudioElement | null>(null);
  const audioPlayingMessageIdRef = useRef<string | null>(null);
  const audioRequestRef = useRef<{ requestId: string; messageId: string } | null>(null);
  const cancelledAudioRequestsRef = useRef<Set<string>>(new Set());
  const abortRequestedRef = useRef(false);
  const abortSoundRef = useRef(false);
  const wasGeneratingRef = useRef(false);
  const autoPlaySignatureRef = useRef<string | null>(null);
  const autoPlayInFlightRef = useRef(false);
  const sendStartSignatureRef = useRef<string | null>(null);
  const sendingPrevRef = useRef(false);
  const previousChatKeyRef = useRef<string | null>(null);

  // Help Me Reply states
  const [showPlusMenu, setShowPlusMenu] = useState(false);
  const [swapPlaces, setSwapPlaces] = useState(false);
  const [showChoiceMenu, setShowChoiceMenu] = useState(false);
  const [showResultMenu, setShowResultMenu] = useState(false);
  const [showPersonaSelector, setShowPersonaSelector] = useState(false);
  const [showAuthorNoteMenu, setShowAuthorNoteMenu] = useState(false);
  const inlineAuthorNoteEnabled = useAuthorNoteInlineEditor();
  const [showBackgroundMenu, setShowBackgroundMenu] = useState(false);
  const [showBackgroundLibraryMenu, setShowBackgroundLibraryMenu] = useState(false);
  const [generatedReply, setGeneratedReply] = useState<string | null>(null);
  const [generatingReply, setGeneratingReply] = useState(false);
  const [helpMeReplyReasoning, setHelpMeReplyReasoning] = useState(false);
  const [helpMeReplyError, setHelpMeReplyError] = useState<string | null>(null);
  const [showScenePromptModeMenu, setShowScenePromptModeMenu] = useState(false);
  const [showScenePromptEditorMenu, setShowScenePromptEditorMenu] = useState(false);
  const [showScenePromptResultMenu, setShowScenePromptResultMenu] = useState(false);
  const [scenePromptTargetMessage, setScenePromptTargetMessage] = useState<StoredMessage | null>(
    null,
  );
  const [scenePromptDraft, setScenePromptDraft] = useState("");
  const [generatedScenePrompt, setGeneratedScenePrompt] = useState<string | null>(null);
  const [generatingScenePrompt, setGeneratingScenePrompt] = useState(false);
  const [scenePromptError, setScenePromptError] = useState<string | null>(null);
  const [applyingSceneImage, setApplyingSceneImage] = useState(false);
  const [scenePromptResultMode, setScenePromptResultMode] = useState<"generated" | "approval">(
    "generated",
  );
  const [helpMeReplyEnabled, setHelpMeReplyEnabled] = useState(true);
  const [sceneGenerationEnabled, setSceneGenerationEnabled] = useState(false);
  const [shouldTriggerFileInput, setShouldTriggerFileInput] = useState(false);
  const [savingSessionBackground, setSavingSessionBackground] = useState(false);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const isMobile = useMemo(() => getPlatform().type === "mobile", []);
  const [settingsDrawerOpen, setSettingsDrawerOpen] = useState(false);
  const footerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const shouldRestoreFooterFocusRef = useRef(false);
  const previousSettingsDrawerOpenRef = useRef(false);
  const helpMeReplyRequestIdRef = useRef<string | null>(null);
  const helpMeReplyUnlistenRef = useRef<UnlistenFn | null>(null);
  const helpMeReplyLoadingTimeoutRef = useRef<number | null>(null);
  const sessionBackgroundInputRef = useRef<HTMLInputElement | null>(null);
  const backgroundLibraryScrollRef = useRef<HTMLDivElement | null>(null);

  const selectedScene =
    chatController.character?.scenes.find(
      (scene) =>
        scene.id ===
        (chatController.session?.selectedSceneId ?? chatController.character?.defaultSceneId),
    ) ?? null;
  const sessionBackgroundPreview = useImageData(chatController.session?.backgroundImagePath);
  const sceneBackgroundPreview = useImageData(selectedScene?.backgroundImagePath);
  const characterBackgroundPreview = useImageData(chatController.character?.backgroundImagePath);
  const hasSessionBackgroundOverride = !!chatController.session?.backgroundImagePath;
  const hasSceneBackgroundDefault = !!selectedScene?.backgroundImagePath;
  const hasCharacterBackgroundDefault = !!chatController.character?.backgroundImagePath;

  const handleImageClick = useCallback((src: string, alt: string) => {
    setSelectedImage({ src, alt });
  }, []);

  const handleUpdateSessionBackground = useCallback(
    async (backgroundImagePath: string | null) => {
      if (!chatController.session?.id) return;
      await updateSessionBackgroundImage(chatController.session.id, backgroundImagePath);
    },
    [chatController.session?.id],
  );

  const handleOpenAuthorNoteMenu = useCallback(() => {
    setShowPlusMenu(false);
    setSettingsDrawerOpen(false);
    setShowAuthorNoteMenu(true);
  }, []);

  const handleSessionBackgroundUpload = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      const input = event.target;
      if (!file) return;

      setSavingSessionBackground(true);
      try {
        const dataUrl = await processBackgroundImage(file);
        const imageRef = await convertToImageRef(dataUrl);
        await handleUpdateSessionBackground(imageRef);
        setShowBackgroundMenu(false);
      } catch (error) {
        console.error("Failed to update session background:", error);
      } finally {
        input.value = "";
        setSavingSessionBackground(false);
      }
    },
    [handleUpdateSessionBackground],
  );

  const handleUseCharacterBackground = useCallback(async () => {
    setSavingSessionBackground(true);
    try {
      await handleUpdateSessionBackground(null);
      setShowBackgroundMenu(false);
    } catch (error) {
      console.error("Failed to revert to character background:", error);
    } finally {
      setSavingSessionBackground(false);
    }
  }, [handleUpdateSessionBackground]);

  const handleUseLibraryBackground = useCallback(
    async (item: ImageLibraryItem) => {
      setSavingSessionBackground(true);
      try {
        const storedId =
          item.bucket === "stored" ? item.filename.replace(/\.[^.]+$/, "") || null : null;
        await handleUpdateSessionBackground(storedId ?? convertFileSrc(item.filePath));
        setShowBackgroundLibraryMenu(false);
        setShowBackgroundMenu(false);
      } catch (error) {
        console.error("Failed to use library background:", error);
      } finally {
        setSavingSessionBackground(false);
      }
    },
    [handleUpdateSessionBackground],
  );

  const selectedImagePrompt = useMemo(() => {
    const value = selectedImage?.alt?.trim();
    if (!value) return null;
    if (value === t("chats.message.attachedImage")) return null;
    return value;
  }, [selectedImage, t]);

  useEffect(() => {
    if (!selectedImage) return;
    setSelectedImagePromptExpanded(false);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setSelectedImage(null);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedImage]);

  useEffect(() => {
    if (showCharacterSelector || showGroupCharacterSelector) {
      listCharacters().then(setAvailableCharacters).catch(console.error);
    }
  }, [showCharacterSelector, showGroupCharacterSelector]);

  useEffect(() => {
    if (!showGroupCharacterSelector || !characterId) return;
    setGroupBranchSelectedIds(new Set([characterId]));
    setGroupBranchError(null);
  }, [showGroupCharacterSelector, characterId]);

  // Reload session data when memories change
  const handleSessionUpdate = useCallback(async () => {
    if (sessionId) {
      const updatedSession = await getSessionMeta(sessionId);
      setSessionForHeader(updatedSession);
    }
  }, [sessionId]);

  useEffect(() => {
    let mounted = true;
    const loadAccessibilitySettings = async () => {
      try {
        const settings = await readSettings();
        const next =
          settings.advancedSettings?.accessibility ?? createDefaultAccessibilitySettings();
        if (mounted) {
          setAccessibilitySettings(next);
          setHelpMeReplyEnabled(settings.advancedSettings?.helpMeReplyEnabled ?? true);
          setSceneGenerationEnabled(settings.advancedSettings?.sceneGenerationEnabled ?? true);
        }
      } catch (error) {
        console.error("Failed to load accessibility settings:", error);
      }
    };

    void loadAccessibilitySettings();
    const listener = () => {
      void loadAccessibilitySettings();
    };
    window.addEventListener(SETTINGS_UPDATED_EVENT, listener);
    window.addEventListener(SESSION_UPDATED_EVENT, handleSessionUpdate);
    return () => {
      mounted = false;
      window.removeEventListener(SETTINGS_UPDATED_EVENT, listener);
      window.removeEventListener(SESSION_UPDATED_EVENT, handleSessionUpdate);
    };
  }, [handleSessionUpdate]);

  useEffect(() => {
    setSessionForHeader(chatController.session);
  }, [chatController.session]);

  useEffect(() => {
    const wasOpen = previousSettingsDrawerOpenRef.current;
    previousSettingsDrawerOpenRef.current = settingsDrawerOpen;
    if (!wasOpen || settingsDrawerOpen || !shouldRestoreFooterFocusRef.current) {
      return;
    }

    shouldRestoreFooterFocusRef.current = false;
    const restoreId = window.requestAnimationFrame(() => {
      footerTextareaRef.current?.focus();
    });

    return () => window.cancelAnimationFrame(restoreId);
  }, [settingsDrawerOpen]);

  const {
    character,
    persona,
    session,
    messages,
    draft,
    setDraft,
    loading,
    sending,
    error,
    setError,
    messageAction,
    setMessageAction,
    actionError,
    setActionError,
    actionStatus,
    setActionStatus,
    actionBusy,
    setActionBusy,
    editDraft,
    setEditDraft,
    heldMessageId,
    setHeldMessageId,
    regeneratingMessageId,
    pendingAttachments,
    addPendingAttachment,
    removePendingAttachment,
    handleSend,
    handleContinue,
    handleRegenerate,
    handleAbort,
    hasMoreMessagesBefore,
    loadOlderMessages,
    ensureMessageLoaded,
    getVariantState,
    handleVariantDrag,
    handleSaveEdit,
    handleDeleteMessage,
    resetMessageActions,
    initializeLongPressTimer,
    isStartingSceneMessage,
    streamingReasoning,
    generateAiScenePrompt,
    applySceneImagePrompt,
  } = chatController;

  const beetrootRain = useBeetrootRain();
  useBeetrootEasterEgg({ messages, fire: beetrootRain.fire });

  const resolveSceneAttachment = useCallback((message: StoredMessage | null | undefined) => {
    if (!message) return null;
    const attachments = message.attachments ?? [];
    return attachments.length > 0 ? attachments[attachments.length - 1] : null;
  }, []);

  const resolveScenePromptSeed = useCallback(
    (message: StoredMessage | null | undefined) => {
      const attachment = resolveSceneAttachment(message);
      const prompt = attachment?.filename?.trim();
      if (!prompt || prompt === t("chats.message.attachedImage")) {
        return "";
      }
      return prompt;
    },
    [resolveSceneAttachment, t],
  );

  const handleToggleGroupBranchCharacter = useCallback(
    (id: string) => {
      if (!character || id === character.id) return;
      setGroupBranchSelectedIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      });
      setGroupBranchError(null);
    },
    [character],
  );

  const handleCreateGroupBranch = useCallback(async () => {
    if (!session || !character || !messageToBranch) return;

    const selectedCharacterIds = Array.from(groupBranchSelectedIds);
    if (!selectedCharacterIds.includes(character.id)) {
      selectedCharacterIds.unshift(character.id);
    }
    if (selectedCharacterIds.length < 2) {
      setGroupBranchError(t("chats.selectTwoCharactersMinimum"));
      return;
    }

    setGroupBranchCreating(true);
    setGroupBranchError(null);
    setActionBusy(true);
    setActionError(null);
    setActionStatus(null);

    try {
      const sourceSession = await getSession(session.id);
      if (!sourceSession) {
        throw new Error("Failed to load source session.");
      }

      if (!sourceSession.messages.some((msg) => msg.id === messageToBranch.id)) {
        throw new Error("Selected message was not found in the session.");
      }

      const selectedSceneId = sourceSession.selectedSceneId ?? character.defaultSceneId;
      const ownerScene = character.scenes.find((scene) => scene.id === selectedSceneId);
      const ownerSceneContent = ownerScene ? resolveSceneContent(ownerScene).trim() : "";
      const startingScene = ownerSceneContent
        ? {
            id: globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`,
            content: ownerSceneContent,
            direction: ownerScene?.direction ?? "",
            createdAt: Date.now(),
          }
        : null;

      const groupSession = await createBranchedGroupSession(sourceSession, messageToBranch.id, {
        name: `${character.name} Branch`,
        characterIds: selectedCharacterIds,
        ownerCharacterId: character.id,
        personaId: sourceSession.personaDisabled ? null : (sourceSession.personaId ?? null),
        startingScene,
        backgroundImagePath:
          sourceSession.backgroundImagePath ??
          ownerScene?.backgroundImagePath ??
          character.backgroundImagePath ??
          null,
      });

      setShowGroupCharacterSelector(false);
      setMessageToBranch(null);
      setActionStatus(t("chats.groupBranchCreated"));
      navigate(`/group-chats/${groupSession.id}`);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setGroupBranchError(message);
      setActionError(message);
    } finally {
      setGroupBranchCreating(false);
      setActionBusy(false);
    }
  }, [
    session,
    character,
    messageToBranch,
    groupBranchSelectedIds,
    setActionBusy,
    setActionError,
    setActionStatus,
    navigate,
  ]);

  const isGenerating = sending || regeneratingMessageId !== null;
  const lastMessageContentLength = messages[messages.length - 1]?.content.length ?? 0;

  useEffect(() => {
    const checkModelCapabilities = async () => {
      if (!character) {
        setSupportsImageInput(false);
        return;
      }
      try {
        const settings = await readSettings();
        const effectiveModelId = character.defaultModelId || settings.defaultModelId;
        const currentModel = settings.models.find((m: Model) => m.id === effectiveModelId);
        const hasImageScope = currentModel?.inputScopes?.includes("image") ?? false;
        setSupportsImageInput(hasImageScope);
      } catch (err) {
        console.error("Failed to check model capabilities:", err);
        setSupportsImageInput(false);
      }
    };
    checkModelCapabilities();
  }, [character]);

  const swapOverlayStyle = useMemo<CSSProperties>(() => {
    const tintRgb = isBackgroundLight ? "22, 101, 52" : "16, 185, 129";
    const edgeAlpha = isBackgroundLight ? 0.045 : 0.08;
    const sideAlpha = isBackgroundLight ? 0.02 : 0.045;
    const topBottomAlpha = isBackgroundLight ? 0.02 : 0.03;
    const baseScrim = isBackgroundLight ? 0.34 : 0.14;
    return {
      background: `
        linear-gradient(rgba(0, 0, 0, ${baseScrim}), rgba(0, 0, 0, ${baseScrim})),
        radial-gradient(132% 102% at 50% 50%, rgba(${tintRgb}, 0) 54%, rgba(${tintRgb}, ${edgeAlpha}) 100%),
        linear-gradient(to right, rgba(${tintRgb}, ${sideAlpha}) 0%, rgba(${tintRgb}, 0) 16%, rgba(${tintRgb}, 0) 84%, rgba(${tintRgb}, ${sideAlpha}) 100%),
        linear-gradient(to bottom, rgba(${tintRgb}, ${topBottomAlpha}) 0%, rgba(${tintRgb}, 0) 20%, rgba(${tintRgb}, 0) 80%, rgba(${tintRgb}, ${topBottomAlpha}) 100%)
      `,
    };
  }, [isBackgroundLight]);

  const ensureAudioProviders = useCallback(async () => {
    if (audioCacheRef.current.providers) {
      return audioCacheRef.current.providers;
    }
    const providers = await listAudioProviders();
    audioCacheRef.current.providers = providers;
    return providers;
  }, []);

  const ensureUserVoices = useCallback(async () => {
    if (audioCacheRef.current.userVoices) {
      return audioCacheRef.current.userVoices;
    }
    const voices = await listUserVoices();
    audioCacheRef.current.userVoices = voices;
    return voices;
  }, []);

  const ensureAudioModels = useCallback(async (providerType: AudioProviderType) => {
    const cached = audioCacheRef.current.modelsByProviderType.get(providerType);
    if (cached) {
      return cached;
    }
    const models = await listAudioModels(providerType);
    audioCacheRef.current.modelsByProviderType.set(providerType, models);
    return models;
  }, []);

  const setAudioStatus = useCallback((messageId: string, status: "loading" | "playing" | null) => {
    setAudioStatusByMessage((prev) => {
      if (status === null) {
        if (!(messageId in prev)) return prev;
        const next = { ...prev };
        delete next[messageId];
        return next;
      }
      if (prev[messageId] === status) return prev;
      return { ...prev, [messageId]: status };
    });
  }, []);

  const buildAudioCacheKey = useCallback(
    (params: {
      providerId: string;
      modelId: string;
      voiceId: string;
      text: string;
      prompt?: string | null;
    }) => {
      const promptKey = params.prompt?.trim() ?? "";
      return [params.providerId, params.modelId, params.voiceId, promptKey, params.text].join("::");
    },
    [],
  );

  const cacheAudioPreview = useCallback((key: string, response: TtsPreviewResponse) => {
    const cache = audioPreviewCacheRef.current;
    cache.set(key, response);
    if (cache.size <= MAX_AUDIO_CACHE_ENTRIES) return;
    const oldestKey = cache.keys().next().value;
    if (oldestKey) {
      cache.delete(oldestKey);
    }
  }, []);

  const startAudioPlayback = useCallback(
    (messageId: string, response: TtsPreviewResponse) => {
      setAudioStatus(messageId, "playing");
      const audio = playAudioFromBase64(response.audioBase64, response.format);
      audioPlaybackRef.current = audio;
      audioPlayingMessageIdRef.current = messageId;
      audio.onended = () => {
        if (audioPlaybackRef.current === audio) {
          audioPlaybackRef.current = null;
          audioPlayingMessageIdRef.current = null;
          setAudioStatus(messageId, null);
        }
      };
      audio.onerror = () => {
        if (audioPlaybackRef.current === audio) {
          audioPlaybackRef.current = null;
          audioPlayingMessageIdRef.current = null;
          setAudioStatus(messageId, null);
        }
      };
    },
    [setAudioStatus],
  );

  const stopAudioPlayback = useCallback(() => {
    const audio = audioPlaybackRef.current;
    if (audio) {
      audio.pause();
      audio.currentTime = 0;
      audio.onended = null;
      audio.onerror = null;
    }
    audioPlaybackRef.current = null;
    const messageId = audioPlayingMessageIdRef.current;
    if (messageId) {
      audioPlayingMessageIdRef.current = null;
      setAudioStatus(messageId, null);
    }
  }, [setAudioStatus]);

  const cancelAudioGeneration = useCallback(async () => {
    const pending = audioRequestRef.current;
    if (!pending) return;
    audioRequestRef.current = null;
    cancelledAudioRequestsRef.current.add(pending.requestId);
    setAudioStatus(pending.messageId, null);
    try {
      await abortAudioPreview(pending.requestId);
    } catch (error) {
      console.warn("Failed to cancel audio preview:", error);
    }
  }, [setAudioStatus]);

  const handleStopAudio = useCallback(
    (message: StoredMessage) => {
      if (audioPlayingMessageIdRef.current && audioPlayingMessageIdRef.current !== message.id) {
        return;
      }
      stopAudioPlayback();
    },
    [stopAudioPlayback],
  );

  const handleCancelAudio = useCallback(
    (message: StoredMessage) => {
      if (audioRequestRef.current && audioRequestRef.current.messageId !== message.id) {
        return;
      }
      void cancelAudioGeneration();
    },
    [cancelAudioGeneration],
  );

  useEffect(() => {
    const chatKey = `${characterId ?? ""}:${sessionId ?? ""}`;
    const previousKey = previousChatKeyRef.current;
    if (previousKey && previousKey !== chatKey) {
      stopAudioPlayback();
      void cancelAudioGeneration();
    }
    previousChatKeyRef.current = chatKey;
  }, [cancelAudioGeneration, characterId, sessionId, stopAudioPlayback]);

  useEffect(() => {
    return () => {
      stopAudioPlayback();
      void cancelAudioGeneration();
    };
  }, [cancelAudioGeneration, stopAudioPlayback]);

  const handlePlayMessageAudio = useCallback(
    async (message: StoredMessage, text: string) => {
      if (message.id.startsWith("placeholder")) return;
      if (message.role !== "assistant" && message.role !== "scene") return;
      if (!character?.voiceConfig) return;

      const trimmedText = text.trim();
      if (!trimmedText) return;

      if (audioRequestRef.current?.messageId === message.id) {
        await cancelAudioGeneration();
        return;
      }
      if (audioPlayingMessageIdRef.current === message.id) {
        stopAudioPlayback();
        return;
      }

      if (audioRequestRef.current) {
        await cancelAudioGeneration();
      }
      if (audioPlaybackRef.current) {
        stopAudioPlayback();
      }

      const requestId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      audioRequestRef.current = { requestId, messageId: message.id };
      setAudioStatus(message.id, "loading");

      let providers: AudioProvider[];
      try {
        providers = await ensureAudioProviders();
      } catch (error) {
        if (audioRequestRef.current?.requestId === requestId) {
          audioRequestRef.current = null;
        }
        setAudioStatus(message.id, null);
        const messageText = error instanceof Error ? error.message : String(error);
        const isAbort =
          messageText.toLowerCase().includes("aborted") ||
          messageText.toLowerCase().includes("cancel");
        if (isAbort) return;
        throw error;
      }

      if (character.voiceConfig.source === "user" && character.voiceConfig.userVoiceId) {
        let voices = await ensureUserVoices();
        let voice = voices.find((v) => v.id === character.voiceConfig?.userVoiceId);
        if (!voice) {
          audioCacheRef.current.userVoices = null;
          voices = await ensureUserVoices();
          voice = voices.find((v) => v.id === character.voiceConfig?.userVoiceId);
        }
        if (!voice) {
          throw new Error("Assigned voice not found.");
        }
        const provider = providers.find((p) => p.id === voice.providerId);
        if (!provider) {
          throw new Error("Assigned provider not found.");
        }

        const cacheKey = buildAudioCacheKey({
          providerId: voice.providerId,
          modelId: voice.modelId,
          voiceId: voice.voiceId,
          text: trimmedText,
          prompt: voice.prompt,
        });
        const cached = audioPreviewCacheRef.current.get(cacheKey);
        if (cached) {
          if (audioRequestRef.current?.requestId !== requestId) {
            cancelledAudioRequestsRef.current.delete(requestId);
            return;
          }
          audioRequestRef.current = null;
          if (cancelledAudioRequestsRef.current.has(requestId)) {
            cancelledAudioRequestsRef.current.delete(requestId);
            setAudioStatus(message.id, null);
            return;
          }
          startAudioPlayback(message.id, cached);
          return;
        }

        try {
          const response = await generateTtsForMessage(
            voice.providerId,
            voice.modelId,
            voice.voiceId,
            trimmedText,
            voice.prompt,
            requestId,
          );
          if (audioRequestRef.current?.requestId !== requestId) {
            cancelledAudioRequestsRef.current.delete(requestId);
            return;
          }
          audioRequestRef.current = null;
          if (cancelledAudioRequestsRef.current.has(requestId)) {
            cancelledAudioRequestsRef.current.delete(requestId);
            setAudioStatus(message.id, null);
            return;
          }
          cacheAudioPreview(cacheKey, response);
          startAudioPlayback(message.id, response);
          return;
        } catch (error) {
          if (audioRequestRef.current?.requestId === requestId) {
            audioRequestRef.current = null;
          }
          setAudioStatus(message.id, null);
          const messageText = error instanceof Error ? error.message : String(error);
          const isAbort =
            messageText.toLowerCase().includes("aborted") ||
            messageText.toLowerCase().includes("cancel");
          if (isAbort) return;
          throw error;
        }
      }

      if (character.voiceConfig.source === "provider") {
        const providerId = character.voiceConfig.providerId;
        const voiceId = character.voiceConfig.voiceId;
        if (!providerId || !voiceId) {
          throw new Error("Voice assignment is missing provider details.");
        }
        const provider = providers.find((p) => p.id === providerId);
        if (!provider) {
          throw new Error("Assigned provider not found.");
        }

        let modelId = character.voiceConfig.modelId;
        if (!modelId) {
          if (provider.providerType === "kokoro" && provider.kokoroVariant) {
            modelId = provider.kokoroVariant;
          } else {
            const models = await ensureAudioModels(provider.providerType as AudioProviderType);
            modelId = models[0]?.id;
          }
        }
        if (!modelId) {
          throw new Error("No audio models available for this provider.");
        }

        const cacheKey = buildAudioCacheKey({
          providerId,
          modelId,
          voiceId,
          text: trimmedText,
        });
        const cached = audioPreviewCacheRef.current.get(cacheKey);
        if (cached) {
          if (audioRequestRef.current?.requestId !== requestId) {
            cancelledAudioRequestsRef.current.delete(requestId);
            return;
          }
          audioRequestRef.current = null;
          if (cancelledAudioRequestsRef.current.has(requestId)) {
            cancelledAudioRequestsRef.current.delete(requestId);
            setAudioStatus(message.id, null);
            return;
          }
          startAudioPlayback(message.id, cached);
          return;
        }

        try {
          const response = await generateTtsForMessage(
            providerId,
            modelId,
            voiceId,
            trimmedText,
            undefined,
            requestId,
          );
          if (audioRequestRef.current?.requestId !== requestId) {
            cancelledAudioRequestsRef.current.delete(requestId);
            return;
          }
          audioRequestRef.current = null;
          if (cancelledAudioRequestsRef.current.has(requestId)) {
            cancelledAudioRequestsRef.current.delete(requestId);
            setAudioStatus(message.id, null);
            return;
          }
          cacheAudioPreview(cacheKey, response);
          startAudioPlayback(message.id, response);
        } catch (error) {
          if (audioRequestRef.current?.requestId === requestId) {
            audioRequestRef.current = null;
          }
          setAudioStatus(message.id, null);
          const messageText = error instanceof Error ? error.message : String(error);
          const isAbort =
            messageText.toLowerCase().includes("aborted") ||
            messageText.toLowerCase().includes("cancel");
          if (isAbort) return;
          throw error;
        }
      }
    },
    [
      buildAudioCacheKey,
      cacheAudioPreview,
      cancelAudioGeneration,
      character,
      ensureAudioModels,
      ensureAudioProviders,
      ensureUserVoices,
      setAudioStatus,
      startAudioPlayback,
      stopAudioPlayback,
    ],
  );

  const effectiveVoiceAutoplay = useMemo(() => {
    return session?.voiceAutoplay ?? character?.voiceAutoplay ?? false;
  }, [character?.voiceAutoplay, session?.voiceAutoplay]);

  const handleAbortWithFlag = useCallback(async () => {
    abortRequestedRef.current = true;
    abortSoundRef.current = true;
    playAccessibilitySound("failure", accessibilitySettings);
    await handleAbort();
  }, [accessibilitySettings, handleAbort]);

  const openMessageActions = useCallback(
    (message: StoredMessage) => {
      setMessageAction({ message, mode: "view" });
      setEditDraft(message.content);
      setActionError(null);
      setActionStatus(null);
      setActionBusy(false);
    },
    [setMessageAction, setEditDraft, setActionError, setActionStatus, setActionBusy],
  );

  const scheduleLongPress = useCallback(
    (message: StoredMessage) => {
      const timer = window.setTimeout(() => {
        initializeLongPressTimer(null);
        openMessageActions(message);
      }, LONG_PRESS_DELAY);
      initializeLongPressTimer(timer);
    },
    [initializeLongPressTimer, openMessageActions],
  );

  const handlePressStart = useCallback(
    (message: StoredMessage) => (event: React.MouseEvent | React.TouchEvent) => {
      if (message.id.startsWith("placeholder")) return;

      const clientX = "touches" in event ? event.touches[0].clientX : event.clientX;
      const clientY = "touches" in event ? event.touches[0].clientY : event.clientY;
      pressStartPosition.current = { x: clientX, y: clientY };

      setHeldMessageId(message.id);
      scheduleLongPress(message);
    },
    [scheduleLongPress, setHeldMessageId],
  );

  const handlePressMove = useCallback(
    (event: React.MouseEvent | React.TouchEvent) => {
      if (!pressStartPosition.current) return;

      const clientX = "touches" in event ? event.touches[0].clientX : event.clientX;
      const clientY = "touches" in event ? event.touches[0].clientY : event.clientY;

      const deltaX = Math.abs(clientX - pressStartPosition.current.x);
      const deltaY = Math.abs(clientY - pressStartPosition.current.y);

      if (deltaX > SCROLL_THRESHOLD || deltaY > SCROLL_THRESHOLD) {
        initializeLongPressTimer(null);
        setHeldMessageId(null);
        pressStartPosition.current = null;
      }
    },
    [initializeLongPressTimer, setHeldMessageId],
  );

  const handlePressEnd = useCallback(() => {
    initializeLongPressTimer(null);
    setHeldMessageId(null);
    pressStartPosition.current = null;
  }, [initializeLongPressTimer, setHeldMessageId]);

  // Help Me Reply handlers
  const handleOpenPlusMenu = useCallback(() => {
    setShowPlusMenu(true);
  }, []);

  const handleOpenPersonaSelector = useCallback(async () => {
    try {
      const personaList = await listPersonas();
      setPersonas(personaList);
    } catch (error) {
      console.error("Failed to load personas:", error);
      setPersonas([]);
    }
    setShowPlusMenu(false);
    setShowPersonaSelector(true);
  }, []);

  const handleChangePersona = useCallback(
    async (personaId: string | null) => {
      if (!session) return;
      try {
        const disablePersona = personaId === null;
        const updatedSession = {
          ...session,
          personaId: disablePersona ? null : personaId,
          personaDisabled: disablePersona,
          updatedAt: Date.now(),
        };
        await saveSession(updatedSession);
        setSessionForHeader(updatedSession);
      } catch (error) {
        console.error("Failed to change persona:", error);
      }
    },
    [session],
  );

  const selectedPersonaId = useMemo(() => {
    if (!session) return undefined;
    if (session.personaDisabled || session.personaId === "") return "";
    if (session.personaId) return session.personaId;
    const defaultPersona = personas.find((p) => p.isDefault);
    return defaultPersona?.id;
  }, [session, personas]);

  const handleEnableSwapPlaces = useCallback(() => {
    setSwapPlaces(true);
    setShowPlusMenu(false);
  }, []);

  const handleDisableSwapPlaces = useCallback(() => {
    setSwapPlaces(false);
  }, []);

  const clearHelpMeReplyRuntime = useCallback(() => {
    if (helpMeReplyLoadingTimeoutRef.current !== null) {
      window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
      helpMeReplyLoadingTimeoutRef.current = null;
    }
    if (helpMeReplyUnlistenRef.current) {
      helpMeReplyUnlistenRef.current();
      helpMeReplyUnlistenRef.current = null;
    }
    helpMeReplyRequestIdRef.current = null;
  }, []);

  const cancelHelpMeReplyGeneration = useCallback(async () => {
    const requestId = helpMeReplyRequestIdRef.current;
    clearHelpMeReplyRuntime();
    setGeneratingReply(false);
    setHelpMeReplyReasoning(false);
    if (!requestId) return;
    try {
      await invoke("abort_request", { requestId });
    } catch (err) {
      console.error("Failed to abort Help Me Reply request:", err);
    }
  }, [clearHelpMeReplyRuntime]);

  const handleCloseHelpMeReplyResultMenu = useCallback(() => {
    setShowResultMenu(false);
    setGeneratedReply(null);
    setHelpMeReplyError(null);
    setHelpMeReplyReasoning(false);
    void cancelHelpMeReplyGeneration();
  }, [cancelHelpMeReplyGeneration]);

  const handleHelpMeReply = useCallback(
    async (mode: "new" | "enrich") => {
      if (!session?.id) return;

      setShowChoiceMenu(false);
      setShowPlusMenu(false);
      setGeneratedReply(null);
      setHelpMeReplyError(null);
      setHelpMeReplyReasoning(false);
      setGeneratingReply(true);
      setShowResultMenu(true);

      const requestId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      helpMeReplyRequestIdRef.current = requestId;
      let streamingText = "";
      let hasStartedStreaming = false;

      // Some reasoning models can spend several seconds thinking before sending reply text.
      helpMeReplyLoadingTimeoutRef.current = window.setTimeout(() => {
        if (!hasStartedStreaming) {
          setHelpMeReplyReasoning(true);
        }
      }, 5000);

      try {
        // Set up streaming listener
        const unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
          if (helpMeReplyRequestIdRef.current !== requestId) return;
          try {
            const payload =
              typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;

            if (payload && payload.type === "delta" && payload.data?.text) {
              // Clear loading state on first streaming chunk
              if (!hasStartedStreaming) {
                hasStartedStreaming = true;
                setGeneratingReply(false);
                setHelpMeReplyReasoning(false);
                if (helpMeReplyLoadingTimeoutRef.current !== null) {
                  window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
                  helpMeReplyLoadingTimeoutRef.current = null;
                }
              }
              streamingText += String(payload.data.text);
              setGeneratedReply(streamingText);
            } else if (payload && payload.type === "reasoning" && payload.data?.text) {
              if (!hasStartedStreaming) {
                hasStartedStreaming = true;
                if (helpMeReplyLoadingTimeoutRef.current !== null) {
                  window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
                  helpMeReplyLoadingTimeoutRef.current = null;
                }
              }
              setGeneratingReply(true);
              setHelpMeReplyReasoning(true);
            } else if (payload && payload.type === "error") {
              const message =
                payload.data?.message ||
                payload.data?.error ||
                payload.message ||
                "Help Me Reply failed.";
              setHelpMeReplyError(String(message));
              setGeneratingReply(false);
              setHelpMeReplyReasoning(false);
              if (helpMeReplyLoadingTimeoutRef.current !== null) {
                window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
                helpMeReplyLoadingTimeoutRef.current = null;
              }
            }
          } catch (err) {
            console.error("Error processing streaming event:", err);
          }
        });
        helpMeReplyUnlistenRef.current = unlistenNormalized;

        const currentDraft = mode === "enrich" && draft.trim() ? draft : undefined;
        const result = await generateUserReply(session.id, currentDraft, requestId, swapPlaces);

        // If we didn't get streaming updates, use the final result
        if (!streamingText.trim()) {
          if (result?.trim()) {
            setGeneratedReply(result);
          } else {
            setHelpMeReplyError("Help Me Reply failed to generate a reply.");
          }
        }

        setGeneratingReply(false);
        setHelpMeReplyReasoning(false);
        if (helpMeReplyLoadingTimeoutRef.current !== null) {
          window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
          helpMeReplyLoadingTimeoutRef.current = null;
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setHelpMeReplyError(message);
        setGeneratingReply(false);
        setHelpMeReplyReasoning(false);
      } finally {
        // Only clear loading if streaming hasn't started yet
        if (!hasStartedStreaming) {
          setGeneratingReply(false);
          setHelpMeReplyReasoning(false);
        }
        if (helpMeReplyRequestIdRef.current === requestId) {
          clearHelpMeReplyRuntime();
        }
      }
    },
    [session?.id, draft, swapPlaces, clearHelpMeReplyRuntime],
  );

  const handleUseReply = useCallback(() => {
    if (generatedReply) {
      setDraft(generatedReply);
    }
    setShowResultMenu(false);
    setGeneratedReply(null);
    setHelpMeReplyError(null);
    setHelpMeReplyReasoning(false);
  }, [generatedReply, setDraft]);

  const handlePlusMenuImageUpload = useCallback(() => {
    setShowPlusMenu(false);
    setShouldTriggerFileInput(true);
  }, []);

  const handlePlusMenuHelpMeReply = useCallback(() => {
    setShowPlusMenu(false);
    if (draft.trim()) {
      // Has draft - show choice menu
      setShowChoiceMenu(true);
    } else {
      // No draft - generate directly
      void handleHelpMeReply("new");
    }
  }, [draft, handleHelpMeReply]);

  const resetScenePromptFlow = useCallback(() => {
    setShowScenePromptModeMenu(false);
    setShowScenePromptEditorMenu(false);
    setShowScenePromptResultMenu(false);
    setScenePromptTargetMessage(null);
    setScenePromptDraft("");
    setGeneratedScenePrompt(null);
    setGeneratingScenePrompt(false);
    setScenePromptError(null);
    setApplyingSceneImage(false);
    setScenePromptResultMode("generated");
  }, []);

  const handleOpenSceneImageFlow = useCallback(
    (message: StoredMessage) => {
      setMessageToBranch(null);
      resetMessageActions();
      setScenePromptTargetMessage(message);
      setScenePromptDraft(resolveScenePromptSeed(message));
      setGeneratedScenePrompt(null);
      setScenePromptError(null);
      setGeneratingScenePrompt(false);
      setApplyingSceneImage(false);
      setScenePromptResultMode("generated");
      setShowScenePromptEditorMenu(false);
      setShowScenePromptResultMenu(false);
      setShowScenePromptModeMenu(true);
    },
    [resetMessageActions, resolveScenePromptSeed],
  );

  const handleGenerateAiScenePrompt = useCallback(async () => {
    if (!scenePromptTargetMessage) return;

    setShowScenePromptModeMenu(false);
    setShowScenePromptEditorMenu(false);
    setShowScenePromptResultMenu(true);
    setScenePromptError(null);
    setGeneratingScenePrompt(true);
    setGeneratedScenePrompt(null);
    setScenePromptResultMode("generated");

    try {
      const prompt = await generateAiScenePrompt(scenePromptTargetMessage.id);
      setGeneratedScenePrompt(prompt);
    } catch (error) {
      setScenePromptError(error instanceof Error ? error.message : String(error));
    } finally {
      setGeneratingScenePrompt(false);
    }
  }, [generateAiScenePrompt, scenePromptTargetMessage]);

  const handleOpenManualScenePromptEditor = useCallback(() => {
    setShowScenePromptModeMenu(false);
    setShowScenePromptResultMenu(false);
    setScenePromptError(null);
    setScenePromptDraft((current) => current || resolveScenePromptSeed(scenePromptTargetMessage));
    setShowScenePromptEditorMenu(true);
  }, [resolveScenePromptSeed, scenePromptTargetMessage]);

  const handleApplySceneImagePrompt = useCallback(
    async (prompt: string) => {
      if (!scenePromptTargetMessage) return;
      const targetMessage = scenePromptTargetMessage;

      resetMessageActions();
      setShowScenePromptModeMenu(false);
      setShowScenePromptEditorMenu(false);
      setShowScenePromptResultMenu(false);
      setApplyingSceneImage(true);
      setScenePromptError(null);

      try {
        await applySceneImagePrompt(targetMessage, prompt);
        resetScenePromptFlow();
      } catch (error) {
        setScenePromptError(error instanceof Error ? error.message : String(error));
        setScenePromptTargetMessage(targetMessage);
        setScenePromptDraft(prompt);
        setShowScenePromptEditorMenu(true);
      } finally {
        setApplyingSceneImage(false);
      }
    },
    [
      applySceneImagePrompt,
      resetMessageActions,
      resetScenePromptFlow,
      resolveSceneAttachment,
      scenePromptTargetMessage,
    ],
  );

  useEffect(
    () => () => {
      void cancelHelpMeReplyGeneration();
    },
    [cancelHelpMeReplyGeneration],
  );

  useEffect(() => {
    const handleScenePromptApproval = (event: Event) => {
      const customEvent = event as CustomEvent<ScenePromptApprovalDetail>;
      if (customEvent.detail.sessionId !== session?.id) return;

      setShowScenePromptModeMenu(false);
      setShowScenePromptEditorMenu(false);
      setScenePromptTargetMessage(customEvent.detail.message);
      setScenePromptDraft(customEvent.detail.scenePrompt);
      setGeneratedScenePrompt(customEvent.detail.scenePrompt);
      setScenePromptError(null);
      setGeneratingScenePrompt(false);
      setApplyingSceneImage(false);
      setScenePromptResultMode("approval");
      setShowScenePromptResultMenu(true);
    };

    window.addEventListener(SCENE_PROMPT_APPROVAL_EVENT, handleScenePromptApproval);
    return () => window.removeEventListener(SCENE_PROMPT_APPROVAL_EVENT, handleScenePromptApproval);
  }, [session?.id]);

  const loadOlderFromDb = useCallback(async () => {
    if (!hasMoreMessagesBefore) return;
    if (loadingOlderRef.current) return;
    const container = scrollContainerRef.current;
    if (!container) return;

    loadingOlderRef.current = true;
    pendingScrollAdjustRef.current = {
      prevScrollTop: container.scrollTop,
      prevScrollHeight: container.scrollHeight,
    };
    try {
      await loadOlderMessages();
    } finally {
      // scroll restore happens in the messages-length effect
    }
  }, [hasMoreMessagesBefore, loadOlderMessages]);

  const updateIsAtBottom = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return null;

    const { scrollTop, clientHeight, scrollHeight } = container;
    const atBottom = scrollTop + clientHeight >= scrollHeight - STICKY_BOTTOM_THRESHOLD_PX;
    isAtBottomRef.current = atBottom;
    setIsAtBottom((prev) => (prev === atBottom ? prev : atBottom));
    return scrollTop;
  }, []);

  const handleScroll = useCallback(() => {
    const scrollTop = updateIsAtBottom();
    if (scrollTop === null) return;

    if (scrollTop <= AUTOLOAD_TOP_THRESHOLD_PX && hasMoreMessagesBefore) {
      void loadOlderFromDb();
    }
  }, [hasMoreMessagesBefore, loadOlderFromDb, updateIsAtBottom]);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = "smooth") => {
    const container = scrollContainerRef.current;
    if (!container) return;
    isAtBottomRef.current = true;
    setIsAtBottom(true);
    container.scrollTo({ top: container.scrollHeight, behavior });
  }, []);

  useEffect(() => {
    if (!isMobile) {
      setKeyboardInset(0);
      return;
    }

    const visualViewport = window.visualViewport;
    let focusTimer: number | null = null;

    const updateKeyboardInset = () => {
      const baseHeight = window.innerHeight;
      const viewportHeight = visualViewport?.height ?? baseHeight;
      const viewportOffsetTop = visualViewport?.offsetTop ?? 0;
      const rawInset = Math.max(0, baseHeight - viewportHeight - viewportOffsetTop);
      const nextInset = rawInset > MOBILE_KEYBOARD_THRESHOLD_PX ? Math.round(rawInset) : 0;

      setKeyboardInset((prev) => (prev === nextInset ? prev : nextInset));

      window.requestAnimationFrame(() => {
        updateIsAtBottom();
        const activeElement = document.activeElement;
        if (activeElement instanceof HTMLTextAreaElement && isAtBottomRef.current) {
          scrollToBottom("auto");
        }
      });
    };

    const handleFocusChange = () => {
      updateKeyboardInset();
      if (focusTimer !== null) {
        window.clearTimeout(focusTimer);
      }
      focusTimer = window.setTimeout(updateKeyboardInset, 180);
    };

    updateKeyboardInset();
    visualViewport?.addEventListener("resize", updateKeyboardInset);
    visualViewport?.addEventListener("scroll", updateKeyboardInset);
    window.addEventListener("resize", updateKeyboardInset);
    document.addEventListener("focusin", handleFocusChange);
    document.addEventListener("focusout", handleFocusChange);

    return () => {
      if (focusTimer !== null) {
        window.clearTimeout(focusTimer);
      }
      visualViewport?.removeEventListener("resize", updateKeyboardInset);
      visualViewport?.removeEventListener("scroll", updateKeyboardInset);
      window.removeEventListener("resize", updateKeyboardInset);
      document.removeEventListener("focusin", handleFocusChange);
      document.removeEventListener("focusout", handleFocusChange);
    };
  }, [isMobile, scrollToBottom, updateIsAtBottom]);

  const handleContextMenu = useCallback(
    (message: StoredMessage) => (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault();
      initializeLongPressTimer(null);
      if (message.id.startsWith("placeholder")) return;
      openMessageActions(message);
      setHeldMessageId(null);
    },
    [initializeLongPressTimer, setHeldMessageId, openMessageActions],
  );

  const closeMessageActions = useCallback(
    (force = false) => {
      if (!force && (actionBusy || messageAction?.mode === "edit")) {
        return;
      }
      resetMessageActions();
    },
    [actionBusy, messageAction?.mode, resetMessageActions],
  );

  const handleSendMessage = useCallback(async () => {
    if (sending) return;
    setError(null);

    const hasContent = draft.trim().length > 0 || pendingAttachments.length > 0;

    if (hasContent) {
      const content = draft.trim();
      playAccessibilitySound("send", accessibilitySettings);
      await handleSend(content, undefined, { swapPlaces });
    } else {
      playAccessibilitySound("send", accessibilitySettings);
      await handleContinue({ swapPlaces });
    }
  }, [
    sending,
    setError,
    draft,
    setDraft,
    handleSend,
    handleContinue,
    pendingAttachments,
    accessibilitySettings,
    swapPlaces,
  ]);

  const captureFooterFocusForDrawer = useCallback(() => {
    shouldRestoreFooterFocusRef.current = document.activeElement === footerTextareaRef.current;
  }, []);

  const handleRegenerateMessage = useCallback(
    async (message: StoredMessage, options?: { guidance?: string }) => {
      await handleRegenerate(message, { swapPlaces, guidance: options?.guidance });
    },
    [handleRegenerate, swapPlaces],
  );

  useLayoutEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      if (isAtBottomRef.current) {
        container.scrollTop = container.scrollHeight;
      }
      updateIsAtBottom();
    });

    return () => window.cancelAnimationFrame(frame);
  }, [messages.length, lastMessageContentLength, isGenerating, updateIsAtBottom]);

  useEffect(() => {
    if (sending && !sendingPrevRef.current) {
      abortRequestedRef.current = false;
      const lastPlayable = [...messages]
        .reverse()
        .find(
          (msg) =>
            (msg.role === "assistant" || msg.role === "scene") &&
            !msg.id.startsWith("placeholder") &&
            msg.content.trim().length > 0,
        );
      if (lastPlayable) {
        sendStartSignatureRef.current = `${lastPlayable.id}:${replacePlaceholders(
          lastPlayable.content,
          character?.name ?? "",
          persona?.title ?? "",
        )}`;
      } else {
        sendStartSignatureRef.current = null;
      }
    }
    const wasSending = sendingPrevRef.current;
    sendingPrevRef.current = sending;

    if (!wasSending || sending) return;
    if (!effectiveVoiceAutoplay) return;
    if (abortRequestedRef.current) {
      abortRequestedRef.current = false;
      return;
    }
    if (autoPlayInFlightRef.current) return;

    const lastPlayable = [...messages]
      .reverse()
      .find(
        (msg) =>
          (msg.role === "assistant" || msg.role === "scene") &&
          !msg.id.startsWith("placeholder") &&
          msg.content.trim().length > 0,
      );

    if (!lastPlayable) return;

    const displayText = replacePlaceholders(
      lastPlayable.content,
      character?.name ?? "",
      persona?.title ?? "",
    );
    const signature = `${lastPlayable.id}:${displayText}`;
    if (signature === sendStartSignatureRef.current) return;
    if (signature === autoPlaySignatureRef.current) return;

    autoPlaySignatureRef.current = signature;
    autoPlayInFlightRef.current = true;
    void handlePlayMessageAudio(lastPlayable, displayText)
      .catch((error) => {
        console.error("Failed to autoplay message audio:", error);
      })
      .finally(() => {
        autoPlayInFlightRef.current = false;
      });
  }, [
    character?.name,
    effectiveVoiceAutoplay,
    handlePlayMessageAudio,
    messages,
    persona?.title,
    sending,
  ]);

  useEffect(() => {
    const wasGenerating = wasGeneratingRef.current;
    if (!wasGenerating && isGenerating) {
      abortSoundRef.current = false;
    }
    if (wasGenerating && !isGenerating) {
      if (abortSoundRef.current) {
        abortSoundRef.current = false;
        return;
      }
      if (error) {
        playAccessibilitySound("failure", accessibilitySettings);
      } else {
        playAccessibilitySound("success", accessibilitySettings);
      }
    }
    wasGeneratingRef.current = isGenerating;
  }, [accessibilitySettings, error, isGenerating]);

  useEffect(() => {
    if (wasGeneratingRef.current || isGenerating) return;
    if (!messages.length) return;
    const lastMsg = messages[messages.length - 1];
    if (lastMsg?.role !== "assistant" || lastMsg.id.startsWith("placeholder")) return;
    const timer = setTimeout(() => triggerPostFirstMessageTour(), 600);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isGenerating]);

  useEffect(() => {
    if (!isAtBottom || !isGenerating) return;
    scrollToBottom("auto");
  }, [isAtBottom, isGenerating, scrollToBottom]);

  useEffect(() => {
    const adjust = pendingScrollAdjustRef.current;
    if (!adjust) return;
    const container = scrollContainerRef.current;
    if (!container) return;

    // Preserve the visible viewport position when prepending older messages.
    const nextScrollHeight = container.scrollHeight;
    const delta = nextScrollHeight - adjust.prevScrollHeight;
    container.scrollTop = adjust.prevScrollTop + delta;
    pendingScrollAdjustRef.current = null;
    loadingOlderRef.current = false;
  }, [messages.length]);

  useEffect(() => {
    if (!jumpToMessageId || loading) return;

    let cancelled = false;
    let rafId: number | null = null;
    let highlightTimeoutId: number | null = null;

    const run = async () => {
      await ensureMessageLoaded(jumpToMessageId);
      if (cancelled) return;

      let tries = 0;
      const tryScroll = () => {
        if (cancelled) return;
        const element = document.getElementById(`message-${jumpToMessageId}`);
        if (element) {
          element.scrollIntoView({ behavior: "smooth", block: "center" });
          element.classList.add("bg-white/10", "rounded-lg", "transition-colors", "duration-1000");
          highlightTimeoutId = window.setTimeout(() => {
            element.classList.remove("bg-white/10");
          }, 2000);
          return;
        }

        tries += 1;
        if (tries < 20) {
          rafId = window.requestAnimationFrame(tryScroll);
        }
      };

      rafId = window.requestAnimationFrame(tryScroll);
    };

    void run();

    return () => {
      cancelled = true;
      if (rafId !== null) {
        window.cancelAnimationFrame(rafId);
      }
      if (highlightTimeoutId !== null) {
        window.clearTimeout(highlightTimeoutId);
      }
    };
  }, [ensureMessageLoaded, jumpToMessageId, loading]);

  if (loading) {
    return <LoadingSpinner />;
  }

  if (backgroundImageLoading && !backgroundImageData) {
    return <LoadingSpinner />;
  }

  if (!character || !session) {
    return <EmptyState title={t("chats.characterNotFound")} />;
  }

  const footerBottomOffset = `calc(env(safe-area-inset-bottom) + ${keyboardInset}px)`;
  const scrollButtonBottomOffset = `calc(env(safe-area-inset-bottom) + ${keyboardInset}px + 88px)`;

  return (
    <div
      className={cn(
        "flex h-screen flex-col overflow-hidden",
        !backgroundImageData && !backgroundImageLoading && "bg-surface",
      )}
    >
      {beetrootRain.overlay}
      <AnimatePresence>
        {swapPlaces && (
          <motion.div
            key="swap-overlay"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="pointer-events-none fixed inset-0 z-25"
            style={swapOverlayStyle}
          />
        )}
      </AnimatePresence>

      {/* Header */}
      <div className="relative z-20">
        <ChatHeader
          character={character}
          persona={persona}
          swapPlaces={swapPlaces}
          sessionId={sessionId}
          session={sessionForHeader}
          hasBackgroundImage={!!backgroundImageData}
          headerOverlayClassName={theme.headerOverlay}
          onSessionUpdate={handleSessionUpdate}
          onBeforeSettingsOpen={!isMobile ? captureFooterFocusForDrawer : undefined}
          onSettingsOpen={!isMobile ? () => setSettingsDrawerOpen(true) : undefined}
        />
      </div>

      <AnimatePresence>
        {swapPlaces && (
          <motion.div
            key="swap-banner"
            initial={{ opacity: 0, y: -10, scale: 0.985 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.985 }}
            transition={{ duration: 0.22, ease: "easeOut" }}
            className="fixed left-1/2 top-[calc(env(safe-area-inset-top)+56px)] z-40 w-[min(560px,calc(100vw-16px))] -translate-x-1/2 px-1"
          >
            <div
              className={cn(
                "flex items-center justify-between gap-2 border border-emerald-300/40 bg-emerald-500/15 px-3 py-2 text-emerald-100 backdrop-blur-md",
                radius.full,
              )}
            >
              <span className="text-sm">Swap places is active</span>
              <button
                type="button"
                onClick={handleDisableSwapPlaces}
                className={cn(
                  "rounded-full border border-emerald-200/40 px-3 py-1 text-xs font-medium text-emerald-50 hover:bg-emerald-100/10",
                )}
              >
                End swap
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Main content area */}
      <main
        ref={scrollContainerRef}
        className="relative z-10 flex-1 overflow-y-auto"
        onScroll={handleScroll}
      >
        <div
          className={`${chatAppearance.messageGap === "tight" ? "space-y-2" : chatAppearance.messageGap === "relaxed" ? "space-y-6" : "space-y-4"} px-3 pb-8 pt-4`}
          style={{
            backgroundColor: backgroundImageData
              ? swapPlaces
                ? isBackgroundLight
                  ? "rgba(5, 12, 11, 0.44)"
                  : "rgba(5, 12, 11, 0.22)"
                : theme.contentOverlay || "transparent"
              : "transparent",
          }}
        >
          {hasMoreMessagesBefore && (
            <div className="flex justify-center">
              <button
                type="button"
                onClick={() => void loadOlderFromDb()}
                className={cn(
                  "px-3 py-1.5 text-xs text-white/70 border border-white/15 bg-white/5 hover:bg-white/10",
                  radius.full,
                )}
              >
                Load earlier messages
              </button>
            </div>
          )}

          <LayoutGroup id="swap-message-layout">
            {messages.map((message, index) => {
              const isSceneMessage = isStartingSceneMessage(message);
              const sourceContent = message.content;
              const renderedMessage =
                swapPlaces && (message.role === "user" || message.role === "assistant")
                  ? {
                      ...message,
                      role: (message.role === "user" ? "assistant" : "user") as
                        | "assistant"
                        | "user",
                    }
                  : {
                      ...message,
                      content: sourceContent,
                    };
              const isAssistant = renderedMessage.role === "assistant";
              const isUser = renderedMessage.role === "user";
              const actionable =
                (isAssistant || isUser || isSceneMessage) && !message.id.startsWith("placeholder");
              // Replace placeholders for display only
              const charName = swapPlaces
                ? (chatController.persona?.title ?? "")
                : (character?.name ?? "");
              const personaName = swapPlaces
                ? (character?.name ?? "")
                : (chatController.persona?.title ?? "");
              const parsed = splitThinkTags(sourceContent);
              const sanitizedContent = sanitizeAssistantSceneDirective(parsed.content).cleanContent;
              const displayContent = replacePlaceholders(sanitizedContent, charName, personaName);
              const combinedReasoning = [message.reasoning ?? "", parsed.reasoning]
                .filter(Boolean)
                .join("\n");
              const eventHandlers = actionable
                ? {
                    onMouseDown: handlePressStart(message),
                    onMouseMove: handlePressMove,
                    onMouseUp: handlePressEnd,
                    onMouseLeave: handlePressEnd,
                    onTouchStart: handlePressStart(message),
                    onTouchMove: handlePressMove,
                    onTouchEnd: handlePressEnd,
                    onTouchCancel: handlePressEnd,
                    onContextMenu: handleContextMenu(message),
                  }
                : {};

              return (
                <motion.div
                  key={message.id}
                  id={`message-${message.id}`}
                  className="scroll-mt-24 transition-colors duration-500"
                  layout="position"
                  transition={{
                    layout: { type: "spring", stiffness: 280, damping: 30, mass: 0.9 },
                    duration: 0.18,
                  }}
                >
                  <ChatMessage
                    key={message.id}
                    message={renderedMessage}
                    index={index}
                    messagesLength={messages.length}
                    heldMessageId={heldMessageId}
                    regeneratingMessageId={regeneratingMessageId}
                    sending={sending}
                    eventHandlers={eventHandlers}
                    getVariantState={getVariantState}
                    handleVariantDrag={handleVariantDrag}
                    handleRegenerate={handleRegenerateMessage}
                    isStartingSceneMessage={isStartingSceneMessage(message)}
                    theme={theme}
                    chatAppearance={chatAppearance}
                    displayContent={displayContent}
                    character={character}
                    persona={persona}
                    audioStatus={audioStatusByMessage[message.id]}
                    onPlayAudio={handlePlayMessageAudio}
                    onStopAudio={handleStopAudio}
                    onCancelAudio={handleCancelAudio}
                    onImageClick={handleImageClick}
                    reasoning={streamingReasoning[message.id] || combinedReasoning || undefined}
                    swapPlaces={swapPlaces}
                  />
                </motion.div>
              );
            })}
          </LayoutGroup>
        </div>
      </main>

      <AnimatePresence>
        {!isAtBottom && (
          <motion.button
            type="button"
            aria-label={t("chats.scrollToBottom")}
            onClick={() => scrollToBottom("smooth")}
            initial={{ opacity: 0, y: 10, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: 0.98 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className={cn(
              "fixed right-3 z-30 flex h-11 w-11 items-center justify-center",
              "border border-white/15 bg-black/40 text-white/80 shadow-lg backdrop-blur-sm",
              "hover:bg-black/55 active:scale-95",
              radius.full,
            )}
            style={{ bottom: scrollButtonBottomOffset }}
          >
            <ChevronDown size={18} />
          </motion.button>
        )}
      </AnimatePresence>

      {/* Footer */}
      <div className="relative z-10" style={{ paddingBottom: footerBottomOffset }}>
        <ChatFooter
          topSlot={
            inlineAuthorNoteEnabled && (sessionForHeader?.id ?? chatController.session?.id) ? (
              <InlineAuthorNoteBar
                session={sessionForHeader ?? chatController.session}
                onSaved={setSessionForHeader}
              />
            ) : undefined
          }
          draft={draft}
          setDraft={setDraft}
          error={error}
          sending={sending}
          character={character}
          onSendMessage={handleSendMessage}
          onAbort={handleAbortWithFlag}
          hasBackgroundImage={!!backgroundImageData}
          footerOverlayClassName={theme.footerOverlay}
          pendingAttachments={pendingAttachments}
          onAddAttachment={supportsImageInput ? addPendingAttachment : undefined}
          onRemoveAttachment={supportsImageInput ? removePendingAttachment : undefined}
          onOpenPlusMenu={handleOpenPlusMenu}
          triggerFileInput={shouldTriggerFileInput}
          onFileInputTriggered={() => setShouldTriggerFileInput(false)}
          textareaRef={footerTextareaRef}
        />
      </div>

      <MessageActionsBottomSheet
        messageAction={messageAction}
        actionError={actionError}
        actionStatus={actionStatus}
        actionBusy={actionBusy}
        editDraft={editDraft}
        messages={messages}
        setEditDraft={setEditDraft}
        closeMessageActions={closeMessageActions}
        setActionError={setActionError}
        setActionStatus={setActionStatus}
        handleSaveEdit={handleSaveEdit}
        handleDeleteMessage={handleDeleteMessage}
        handleRewindToMessage={chatController.handleRewindToMessage}
        handleBranchFromMessage={async (message) => {
          const newSessionId = await chatController.handleBranchFromMessage(message);
          if (newSessionId && characterId) {
            navigate(`/chat/${characterId}?sessionId=${newSessionId}`);
          }
          return newSessionId;
        }}
        onBranchToCharacter={(message) => {
          setMessageToBranch(message);
          closeMessageActions(true);
          setShowCharacterSelector(true);
        }}
        onBranchToGroupChat={(message) => {
          setMessageToBranch(message);
          closeMessageActions(true);
          setShowGroupCharacterSelector(true);
        }}
        handleTogglePin={chatController.handleTogglePin}
        setMessageAction={setMessageAction}
        onOpenSceneImageFlow={handleOpenSceneImageFlow}
        hasSceneImage={Boolean(resolveSceneAttachment(messageAction?.message))}
        sceneGenerationEnabled={sceneGenerationEnabled}
        characterMemoryType={character?.memoryType}
        characterDefaultModelId={character?.defaultModelId ?? null}
        characterId={characterId}
        sessionId={session?.id ?? null}
        isCompanionChat={(session?.mode ?? character?.mode) === "companion"}
      />

      {/* Character Selection for Branch */}
      <BottomMenu
        isOpen={showCharacterSelector}
        onClose={() => {
          setShowCharacterSelector(false);
          setMessageToBranch(null);
        }}
        title={t("chats.selectCharacter")}
      >
        <div className="space-y-2 max-h-[60vh] overflow-y-auto">
          <p className="text-sm text-white/50 mb-4">
            Choose a character to continue this conversation with:
          </p>
          {availableCharacters
            .filter((c) => c.id !== characterId)
            .map((char) => (
              <CharacterOption
                key={char.id}
                character={char}
                onClick={async () => {
                  if (!messageToBranch) return;
                  const result = await chatController.handleBranchToCharacter(
                    messageToBranch,
                    char.id,
                  );
                  if (result) {
                    setShowCharacterSelector(false);
                    setMessageToBranch(null);
                    navigate(`/chat/${result.characterId}?sessionId=${result.sessionId}`);
                  }
                }}
              />
            ))}
          {availableCharacters.filter((c) => c.id !== characterId).length === 0 && (
            <p className="text-center text-white/40 py-8">
              No other characters available. Create more characters first.
            </p>
          )}
        </div>
      </BottomMenu>

      {/* Character Selection for Group Branch */}
      <BottomMenu
        isOpen={showGroupCharacterSelector}
        onClose={() => {
          if (groupBranchCreating) return;
          setShowGroupCharacterSelector(false);
          setMessageToBranch(null);
          setGroupBranchError(null);
        }}
        title={t("chats.branchToGroupChat")}
      >
        <div className="space-y-3">
          <p className="text-sm text-white/50">
            Branch owner is locked. Choose additional characters, then create.
          </p>

          {groupBranchError && (
            <div
              className={cn(
                "border border-red-400/25 bg-red-500/10 px-3 py-2 text-sm text-red-200",
                radius.md,
              )}
            >
              {groupBranchError}
            </div>
          )}

          <div className="space-y-2 max-h-[48vh] overflow-y-auto">
            {availableCharacters.map((char) => {
              const isOwner = char.id === character?.id;
              return (
                <CharacterOption
                  key={char.id}
                  character={char}
                  selected={groupBranchSelectedIds.has(char.id)}
                  locked={isOwner}
                  onClick={() => handleToggleGroupBranchCharacter(char.id)}
                />
              );
            })}
          </div>

          <button
            onClick={() => void handleCreateGroupBranch()}
            disabled={groupBranchCreating}
            className={cn(
              "w-full rounded-xl border px-4 py-3 text-sm font-semibold transition",
              "border-emerald-400/35 bg-emerald-500/20 text-emerald-100 hover:bg-emerald-500/30",
              "disabled:cursor-not-allowed disabled:opacity-55",
            )}
          >
            {groupBranchCreating ? "Creating..." : "Create Group Branch"}
          </button>
        </div>
      </BottomMenu>

      {/* Plus Menu - Upload Image | Help Me Reply */}
      <BottomMenu
        isOpen={showPlusMenu}
        onClose={() => setShowPlusMenu(false)}
        title={t("chats.addContent")}
      >
        <div className="space-y-2">
          <MenuButton
            icon={User}
            title={t("chats.settings.persona")}
            description={persona?.title ?? "No persona"}
            onClick={() => {
              void handleOpenPersonaSelector();
            }}
          />
          <MenuButton
            icon={NotebookPen}
            title="Author Note"
            description={
              (sessionForHeader?.authorNote ?? chatController.session?.authorNote)?.trim()
                ? "Active for this chat"
                : "Private direction for replies"
            }
            onClick={handleOpenAuthorNoteMenu}
          />
          <MenuButton
            icon={Image}
            title="Chat Background"
              description={
              hasSessionBackgroundOverride
                ? "Session override active"
                : hasSceneBackgroundDefault
                  ? "Using scene background"
                : hasCharacterBackgroundDefault
                  ? "Using character default background"
                  : "No background selected"
            }
            onClick={() => {
              setShowPlusMenu(false);
              setShowBackgroundMenu(true);
            }}
          />
          <MenuButton
            icon={ArrowLeftRight}
            title={swapPlaces ? t("chats.swapPlacesOn") : t("chats.swapPlaces")}
            description={
              swapPlaces
                ? "You are chatting as the character. Tap top banner to end."
                : "Temporarily chat as the character and let AI reply as your persona."
            }
            onClick={
              swapPlaces
                ? () => {
                    handleDisableSwapPlaces();
                    setShowPlusMenu(false);
                  }
                : handleEnableSwapPlaces
            }
          />
          {supportsImageInput && (
            <MenuButton
              icon={Image}
              title={t("chats.uploadImage")}
              onClick={handlePlusMenuImageUpload}
            />
          )}
          {helpMeReplyEnabled && (
            <MenuButton
              icon={Sparkles}
              title={t("chats.helpMeReply")}
              description="Let AI suggest what to say"
              onClick={handlePlusMenuHelpMeReply}
            />
          )}
        </div>
      </BottomMenu>

      <AuthorNoteBottomMenu
        isOpen={showAuthorNoteMenu}
        onClose={() => setShowAuthorNoteMenu(false)}
        session={sessionForHeader ?? chatController.session}
        onSaved={setSessionForHeader}
      />

      <PersonaSelector
        isOpen={showPersonaSelector}
        onClose={() => setShowPersonaSelector(false)}
        personas={personas}
        selectedPersonaId={selectedPersonaId}
        onSelect={handleChangePersona}
      />

      <BottomMenu
        isOpen={showBackgroundMenu}
        onClose={() => !savingSessionBackground && setShowBackgroundMenu(false)}
        title="Chat Background"
      >
        <div className="space-y-4">
          <input
            ref={sessionBackgroundInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={(event) => {
              void handleSessionBackgroundUpload(event);
            }}
          />

          {(backgroundImageData ||
            sceneBackgroundPreview ||
            characterBackgroundPreview ||
            sessionBackgroundPreview) && (
            <div className="space-y-3">
              {backgroundImageData && (
                <div className="overflow-hidden rounded-2xl border border-fg/10 bg-fg/4">
                  <img
                    src={backgroundImageData}
                    alt="Current chat background"
                    className="h-32 w-full object-cover"
                  />
                  <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/70">
                    {hasSessionBackgroundOverride
                      ? "Current session background"
                      : hasSceneBackgroundDefault
                        ? "Current scene background"
                        : "Current character default background"}
                  </div>
                </div>
              )}

              {hasSessionBackgroundOverride &&
                sceneBackgroundPreview &&
                sceneBackgroundPreview !== sessionBackgroundPreview && (
                  <div className="overflow-hidden rounded-2xl border border-fg/10 bg-fg/4">
                    <img
                      src={sceneBackgroundPreview}
                      alt="Scene background"
                      className="h-24 w-full object-cover"
                    />
                    <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/55">
                      Scene background
                    </div>
                  </div>
                )}

              {hasSessionBackgroundOverride &&
                characterBackgroundPreview &&
                characterBackgroundPreview !== sessionBackgroundPreview &&
                characterBackgroundPreview !== sceneBackgroundPreview && (
                  <div className="overflow-hidden rounded-2xl border border-fg/10 bg-fg/4">
                    <img
                      src={characterBackgroundPreview}
                      alt="Character default background"
                      className="h-24 w-full object-cover"
                    />
                    <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/55">
                      Character default background
                    </div>
                  </div>
                )}
            </div>
          )}

          <div className="space-y-2">
            <MenuButton
              icon={Image}
              title={hasSessionBackgroundOverride ? "Replace Session Background" : "Upload Session Background"}
              description="Only changes this chat session"
              loading={savingSessionBackground}
              onClick={() => sessionBackgroundInputRef.current?.click()}
            />
            <MenuButton
              icon={Image}
              title="Choose from Library"
              description="Pick an existing image library item for this chat session"
              loading={savingSessionBackground}
              onClick={() => setShowBackgroundLibraryMenu(true)}
            />
            {(hasSessionBackgroundOverride ||
              hasSceneBackgroundDefault ||
              hasCharacterBackgroundDefault) && (
              <MenuButton
                icon={X}
                title={
                  hasSceneBackgroundDefault
                    ? "Use Scene Default"
                    : hasCharacterBackgroundDefault
                      ? "Use Character Default"
                      : "Remove Background"
                }
                description={
                  hasSceneBackgroundDefault
                    ? "Clear the session override and return to the current scene background"
                    : hasCharacterBackgroundDefault
                      ? "Clear the session override and return to the character background"
                    : "Remove the session background override"
                }
                loading={savingSessionBackground}
                onClick={() => {
                  void handleUseCharacterBackground();
                }}
              />
            )}
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showBackgroundLibraryMenu}
        onClose={() => !savingSessionBackground && setShowBackgroundLibraryMenu(false)}
        title="Choose Background"
      >
        <div ref={backgroundLibraryScrollRef} className="max-h-[60vh] overflow-y-auto">
          <ImageLibraryPanel
            scrollContainerRef={backgroundLibraryScrollRef}
            embedded
            mode="picker"
            fixedFilter="Backgrounds"
            hideFilterTabs
            columnCountOverride={2}
            onUseItem={(item) => void handleUseLibraryBackground(item)}
          />
        </div>
      </BottomMenu>

      {/* Choice Menu - Use existing draft or generate new */}
      <BottomMenu
        isOpen={showChoiceMenu}
        onClose={() => setShowChoiceMenu(false)}
        title={t("chats.helpMeReply")}
      >
        <div className="space-y-2">
          <p className="text-sm text-white/60 mb-4">
            You have a draft message. How would you like to proceed?
          </p>
          <MenuButton
            icon={PenLine}
            title={t("chats.useMyTextAsBase")}
            description="Expand and improve your draft"
            onClick={() => handleHelpMeReply("enrich")}
          />
          <MenuButton
            icon={Sparkles}
            title={t("chats.writeNewReply")}
            description="Generate a fresh reply"
            onClick={() => handleHelpMeReply("new")}
          />
        </div>
      </BottomMenu>

      {/* Result Menu - Show generated reply with Regenerate/Use options */}
      <BottomMenu
        isOpen={showResultMenu}
        onClose={handleCloseHelpMeReplyResultMenu}
        title={t("chats.suggestedReply")}
      >
        <div className="space-y-4">
          {helpMeReplyError ? (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4">
              <p className="text-red-400 text-sm">{helpMeReplyError}</p>
            </div>
          ) : generatingReply && !generatedReply ? (
            <div className="flex flex-col items-center justify-center gap-3 py-8" role="status">
              <LoadingSpinner />
              <p className="text-center text-sm text-white/60">
                {helpMeReplyReasoning
                  ? "Reasoning before writing your reply..."
                  : "Writing your reply..."}
              </p>
            </div>
          ) : generatedReply ? (
            <div
              className={cn(
                "bg-white/5 border border-white/10 p-4",
                radius.lg,
                "max-h-[40vh] overflow-y-auto",
              )}
            >
              <p className="text-white/90 text-sm whitespace-pre-wrap">{generatedReply}</p>
            </div>
          ) : null}

          {/* Always show buttons, but with appropriate disabled states */}
          <div className="flex gap-3">
            <button
              onClick={() => handleHelpMeReply(draft.trim() ? "enrich" : "new")}
              disabled={generatingReply}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-3 px-4",
                radius.lg,
                "bg-white/10 text-white/80 hover:bg-white/15",
                "disabled:opacity-50 transition-all",
              )}
            >
              <RefreshCw size={18} />
              <span>Regenerate</span>
            </button>
            <button
              onClick={handleUseReply}
              disabled={generatingReply || !generatedReply}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-3 px-4",
                radius.lg,
                "bg-emerald-500 text-white hover:bg-emerald-600",
                "disabled:opacity-50 transition-all",
              )}
            >
              <Check size={18} />
              <span>Use This</span>
            </button>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showScenePromptModeMenu}
        onClose={resetScenePromptFlow}
        title={t("chats.sceneImage.modeTitle")}
      >
        <div className="space-y-2">
          <p className="mb-4 text-sm text-white/60">{t("chats.sceneImage.modeDescription")}</p>
          <MenuButton
            icon={PenLine}
            title={t("chats.sceneImage.writePrompt")}
            description={t("chats.sceneImage.writePromptDesc")}
            onClick={handleOpenManualScenePromptEditor}
          />
          <MenuButton
            icon={Sparkles}
            title={t("chats.sceneImage.askAi")}
            description={t("chats.sceneImage.askAiDesc")}
            onClick={() => void handleGenerateAiScenePrompt()}
          />
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showScenePromptEditorMenu}
        onClose={resetScenePromptFlow}
        title={
          resolveSceneAttachment(scenePromptTargetMessage)
            ? t("chats.sceneImage.regenerateTitle")
            : t("chats.sceneImage.generateTitle")
        }
      >
        <div className="space-y-4">
          {scenePromptError && (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4">
              <p className="text-sm text-red-300">{scenePromptError}</p>
            </div>
          )}
          <div className={cn("border border-white/10 bg-white/5 p-3", radius.lg)}>
            <div className="mb-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-white/45">
              {t("chats.sceneImage.promptLabel")}
            </div>
            <textarea
              value={scenePromptDraft}
              onChange={(event) => setScenePromptDraft(event.target.value)}
              rows={8}
              className={cn(
                "min-h-45 w-full resize-none bg-transparent text-sm leading-relaxed text-white placeholder-white/30 outline-none",
              )}
              placeholder={t("chats.sceneImage.promptPlaceholder")}
              disabled={applyingSceneImage}
              autoFocus
            />
          </div>
          <div className="flex gap-3">
            <button
              onClick={resetScenePromptFlow}
              disabled={applyingSceneImage}
              className={cn(
                "flex-1 px-4 py-3 text-sm font-medium text-white/75 transition",
                "border border-white/10 bg-white/5 hover:bg-white/10 hover:text-white",
                "disabled:opacity-50",
                radius.lg,
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={() => void handleApplySceneImagePrompt(scenePromptDraft)}
              disabled={applyingSceneImage || !scenePromptDraft.trim()}
              className={cn(
                "flex flex-1 items-center justify-center gap-2 px-4 py-3 text-sm font-semibold text-white transition",
                "bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50",
                radius.lg,
              )}
            >
              {applyingSceneImage ? <LoadingSpinner /> : <Image size={18} />}
              <span>
                {resolveSceneAttachment(scenePromptTargetMessage)
                  ? t("chats.sceneImage.updateImage")
                  : t("chats.sceneImage.generateImage")}
              </span>
            </button>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showScenePromptResultMenu}
        onClose={resetScenePromptFlow}
        title={
          scenePromptResultMode === "approval"
            ? t("chats.sceneImage.reviewTitle")
            : t("chats.sceneImage.aiTitle")
        }
      >
        <div className="space-y-4">
          {scenePromptError ? (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4">
              <p className="text-sm text-red-300">{scenePromptError}</p>
            </div>
          ) : generatingScenePrompt ? (
            <div className="flex items-center justify-center py-8">
              <LoadingSpinner />
            </div>
          ) : generatedScenePrompt ? (
            <div className={cn("border border-white/10 bg-white/5 p-4", radius.lg)}>
              <div className="mb-2 flex items-center justify-between gap-3">
                <div className="text-[11px] font-semibold uppercase tracking-[0.22em] text-white/45">
                  {t("chats.sceneImage.suggestedPrompt")}
                </div>
                <div className="text-[11px] text-white/35">
                  {generatedScenePrompt.length.toLocaleString()} chars
                </div>
              </div>
              <p className="line-clamp-6 whitespace-pre-wrap text-sm leading-relaxed text-white/82">
                {generatedScenePrompt}
              </p>
              <p className="mt-3 text-xs text-white/45">
                Use {t("chats.sceneImage.editPrompt").toLowerCase()} to review or change the full prompt.
              </p>
            </div>
          ) : null}

          {scenePromptResultMode === "approval" ? (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <button
                onClick={resetScenePromptFlow}
                disabled={generatingScenePrompt || applyingSceneImage}
                className={cn(
                  "flex items-center justify-center gap-2 px-4 py-3 text-sm font-medium text-white/80 transition",
                  "border border-white/10 bg-white/5 hover:bg-white/10",
                  "disabled:opacity-50",
                  radius.lg,
                )}
              >
                <X size={18} />
                <span>{t("chats.sceneImage.denyPrompt")}</span>
              </button>
              <button
                onClick={() => {
                  setScenePromptDraft(generatedScenePrompt ?? "");
                  setShowScenePromptResultMenu(false);
                  setShowScenePromptEditorMenu(true);
                }}
                disabled={generatingScenePrompt || !generatedScenePrompt || applyingSceneImage}
                className={cn(
                  "flex items-center justify-center gap-2 px-4 py-3 text-sm font-medium text-white/80 transition",
                  "border border-white/10 bg-white/5 hover:bg-white/10",
                  "disabled:opacity-50",
                  radius.lg,
                )}
              >
                <PenLine size={18} />
                <span>{t("chats.sceneImage.editPrompt")}</span>
              </button>
              <button
                onClick={() => void handleApplySceneImagePrompt(generatedScenePrompt ?? "")}
                disabled={generatingScenePrompt || !generatedScenePrompt || applyingSceneImage}
                className={cn(
                  "flex items-center justify-center gap-2 px-4 py-3 text-sm font-semibold text-white transition",
                  "bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50",
                  radius.lg,
                )}
              >
                {applyingSceneImage ? <LoadingSpinner /> : <Check size={18} />}
                <span>{t("chats.sceneImage.acceptPrompt")}</span>
              </button>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <button
                onClick={() => void handleGenerateAiScenePrompt()}
                disabled={generatingScenePrompt || applyingSceneImage}
                className={cn(
                  "flex items-center justify-center gap-2 px-4 py-3 text-sm font-medium text-white/80 transition",
                  "border border-white/10 bg-white/5 hover:bg-white/10",
                  "disabled:opacity-50",
                  radius.lg,
                )}
              >
                <RefreshCw size={18} />
                <span>{t("chats.sceneImage.regeneratePrompt")}</span>
              </button>
              <button
                onClick={() => {
                  setScenePromptDraft(generatedScenePrompt ?? "");
                  setShowScenePromptResultMenu(false);
                  setShowScenePromptEditorMenu(true);
                }}
                disabled={generatingScenePrompt || !generatedScenePrompt || applyingSceneImage}
                className={cn(
                  "flex items-center justify-center gap-2 px-4 py-3 text-sm font-medium text-white/80 transition",
                  "border border-white/10 bg-white/5 hover:bg-white/10",
                  "disabled:opacity-50",
                  radius.lg,
                )}
              >
                <PenLine size={18} />
                <span>{t("chats.sceneImage.editPrompt")}</span>
              </button>
              <button
                onClick={() => void handleApplySceneImagePrompt(generatedScenePrompt ?? "")}
                disabled={generatingScenePrompt || !generatedScenePrompt || applyingSceneImage}
                className={cn(
                  "flex items-center justify-center gap-2 px-4 py-3 text-sm font-semibold text-white transition",
                  "bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50",
                  radius.lg,
                )}
              >
                {applyingSceneImage ? <LoadingSpinner /> : <Check size={18} />}
                <span>
                  {resolveSceneAttachment(scenePromptTargetMessage)
                    ? t("chats.sceneImage.updateImage")
                    : t("chats.sceneImage.generateImage")}
                </span>
              </button>
            </div>
          )}
        </div>
      </BottomMenu>

      {/* Full-screen Image Modal */}
      <AnimatePresence>
        {selectedImage && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-100 flex items-center justify-center bg-black/95 p-4"
            onClick={() => setSelectedImage(null)}
          >
            <motion.button
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.8 }}
              className="absolute right-6 top-10 z-101 flex h-10 w-11 items-center justify-center rounded-full bg-red-500 text-white hover:bg-red-400"
              onClick={() => setSelectedImage(null)}
            >
              <X size={24} />
            </motion.button>
            <motion.div
              initial={{ opacity: 0, scale: 0.96, y: 12 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.96, y: 12 }}
              transition={{ duration: 0.22 }}
              className="relative z-10 flex max-h-[92vh] w-full max-w-[min(94vw,1380px)] flex-col items-center gap-4 lg:grid lg:grid-cols-[minmax(280px,340px)_minmax(0,1fr)] lg:items-stretch lg:gap-6"
              onClick={(e) => e.stopPropagation()}
            >
              {selectedImagePrompt && (
                <motion.div
                  initial={{ opacity: 0, x: -12 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -8 }}
                  transition={{ duration: 0.18, delay: 0.04 }}
                  className="hidden w-full max-w-3xl rounded-4xl border border-white/12 bg-white/4.5 p-3 backdrop-blur-xl lg:order-0 lg:flex lg:h-full lg:max-w-none lg:flex-col lg:rounded-[38px] lg:border-white/10 lg:bg-white/3 lg:p-4"
                >
                  <div className="rounded-[18px] border border-white/10 bg-black/35 px-4 py-3 lg:flex lg:h-full lg:flex-col lg:rounded-3xl lg:border-white/8 lg:bg-black/30 lg:px-5 lg:py-5">
                    <div className="mb-2 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-white/45 lg:mb-4 lg:text-[10px] lg:tracking-[0.28em]">
                      <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-300/80" />
                      Image Prompt
                    </div>
                    <p className="max-h-[52vh] overflow-y-auto pr-1 text-sm leading-relaxed text-white/82 lg:max-h-[72vh] lg:pr-2 lg:text-[15px] lg:leading-7">
                      {selectedImagePrompt}
                    </p>
                  </div>
                </motion.div>
              )}

              <motion.img
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                transition={{ duration: 0.2 }}
                src={selectedImage.src}
                alt={selectedImage.alt}
                className="max-h-[78vh] max-w-full rounded-[28px] object-contain shadow-[0_30px_80px_rgba(0,0,0,0.45)] lg:justify-self-center lg:max-h-[90vh] lg:max-w-[min(100%,920px)]"
              />

              {selectedImagePrompt && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 8 }}
                  transition={{ duration: 0.18, delay: 0.04 }}
                  className="w-full max-w-3xl rounded-3xl border border-white/12 bg-white/4.5 p-3 backdrop-blur-xl lg:hidden"
                >
                  <button
                    type="button"
                    onClick={() => setSelectedImagePromptExpanded((current) => !current)}
                    className="flex w-full items-center justify-between gap-3 rounded-[18px] border border-white/10 bg-black/35 px-4 py-3 text-left"
                  >
                    <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-white/45">
                      <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-300/80" />
                      Image Prompt
                    </div>
                    <ChevronDown
                      size={18}
                      className={cn(
                        "shrink-0 text-white/55 transition-transform duration-200",
                        selectedImagePromptExpanded && "rotate-180",
                      )}
                    />
                  </button>

                  <AnimatePresence initial={false}>
                    {selectedImagePromptExpanded && (
                      <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: "auto" }}
                        exit={{ opacity: 0, height: 0 }}
                        transition={{ duration: 0.18 }}
                        className="overflow-hidden"
                      >
                        <div className="mt-3 rounded-[18px] border border-white/10 bg-black/35 px-4 py-3">
                          <p className="max-h-[26vh] overflow-y-auto pr-1 text-sm leading-relaxed text-white/82">
                            {selectedImagePrompt}
                          </p>
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </motion.div>
              )}
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Desktop Settings Drawer */}
      {!isMobile && character && (
        <ChatSettingsDrawer
          isOpen={settingsDrawerOpen}
          onClose={() => setSettingsDrawerOpen(false)}
          character={character}
          onOpenAuthorNote={handleOpenAuthorNoteMenu}
        />
      )}

      {showChatDetailTour && character && (
        <GuidedTour tour="chatDetail" onDismiss={dismissChatDetailTour} />
      )}
      {showPostFirstMessageTour && !showChatDetailTour && character && (
        <GuidedTour tour="postFirstMessage" onDismiss={dismissPostFirstMessageTour} />
      )}
    </div>
  );
}

function resolveSceneContent(scene: Scene): string {
  if (scene.selectedVariantId) {
    const selectedVariant = scene.variants?.find(
      (variant) => variant.id === scene.selectedVariantId,
    );
    if (selectedVariant?.content?.trim()) {
      return selectedVariant.content;
    }
  }
  return scene.content ?? "";
}

function CharacterOption({
  character,
  onClick,
  selected = false,
  locked = false,
}: {
  character: Character;
  onClick: () => void;
  selected?: boolean;
  locked?: boolean;
}) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  return (
    <button
      onClick={onClick}
      disabled={locked}
      className={cn(
        "flex w-full items-center gap-3 p-3 text-left transition",
        radius.lg,
        selected ? "border-emerald-400/35 bg-emerald-500/10" : "border border-white/10 bg-white/5",
        !locked && "hover:border-white/20 hover:bg-white/10 active:scale-[0.99]",
        locked && "cursor-not-allowed opacity-90",
      )}
    >
      <div className={cn("h-10 w-10 overflow-hidden shrink-0", radius.full, "bg-white/10")}>
        {avatarUrl ? (
          <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
        ) : (
          <div className="h-full w-full flex items-center justify-center text-white/50 font-semibold">
            {character.name.charAt(0).toUpperCase()}
          </div>
        )}
      </div>
      <div className="flex-1 min-w-0">
        <h3 className="text-sm font-medium text-white truncate">{character.name}</h3>
        <p className="text-xs text-white/50 truncate">
          {character.description || character.definition || "No description"}
        </p>
      </div>
      {selected && (
        <div
          className={cn(
            "flex h-7 w-7 items-center justify-center",
            radius.full,
            "bg-emerald-500/20 text-emerald-200",
          )}
        >
          {locked ? <Lock size={13} /> : <Check size={14} />}
        </div>
      )}
    </button>
  );
}
