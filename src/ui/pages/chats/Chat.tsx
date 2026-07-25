import {
  CSSProperties,
  Fragment,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useParams, useSearchParams, useNavigate, useLocation } from "react-router-dom";
import { Routes } from "../../navigation";
import {
  buildAvatarLibrarySelectionKey,
  type AvatarLibrarySelectionPayload,
} from "../../components/AvatarPicker/librarySelection";
import {
  patchWidgetNode,
  setLibraryImageOnNode,
  setScratchPadContentOnNode,
} from "./components/widgets/editor/widgetFactories";
import type { WidgetNode } from "../../../core/storage/chatWidgetSchemas";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import { ArrowLeftRight, ChevronDown, Dices, NotebookPen, Pencil, User, X } from "lucide-react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AccessibilitySettings,
  Character,
  CompanionTimeOverride,
  Model,
  Persona,
  Scene,
  StoredMessage,
} from "../../../core/storage/schemas";
import {
  createDefaultAccessibilitySettings,
  CompanionSessionStateSchema,
} from "../../../core/storage/schemas";
import {
  abortAudioPreview,
  generateTtsForMessage,
  generateTtsPreview,
  listAudioModels,
  listAudioProviders,
  listUserVoices,
  playAudioFromBase64,
  splitTextForTtsQueue,
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
  listBranchTree,
  listCharacters,
  listPersonas,
  readSettings,
  saveSession,
  updateSessionAuthorNote,
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
import { ChatAppearanceDrawer } from "./components/appearance/ChatAppearanceDrawer";
import { CompanionTimeOverrideCard } from "./components/CompanionTimeOverrideCard";
import { getChatColumnLayout } from "./utils/chatColumnLayout";
import { getChatWidgetLayout, useViewportWidth } from "./utils/chatWidgetLayout";
import { ChatWidgetArea } from "./components/ChatWidgetArea";
import { WidgetDice } from "./components/widgets/WidgetDice";
import {
  WidgetContextProvider,
  WidgetEditProvider,
  type WidgetActionContext,
  type WidgetSlots,
  type WidgetEditRestore,
} from "./components/widgets";
import {
  getCharacter,
  saveCharacter,
  updateCharacterChatAppearance,
} from "../../../core/storage";
import { BottomMenu, GuidedTour, MenuButton, useGuidedTour } from "../../components";
import { AvatarImage } from "../../components/AvatarImage";
import { DynamicMemoryApprovalGate } from "../../components/DynamicMemoryApprovalGate";
import { useAvatar } from "../../hooks/useAvatar";
import { Image, RefreshCw, Sparkles, Check, PenLine, Lock, FileAudio, GitBranch, History } from "lucide-react";
import { radius, cn, typography } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { PersonaSelector } from "../group-chats/components/settings";
import { sanitizeAssistantSceneDirective } from "./hooks/sceneImageProtocol";
import { useBeetrootRain } from "./components/BeetrootRain";
import { useBeetrootEasterEgg } from "./hooks/useBeetrootEasterEgg";
import { processBackgroundImage } from "../../../core/utils/image";
import { convertFilePathToDataUrl, convertToImageRef } from "../../../core/storage/images";
import { useImageData } from "../../hooks/useImageData";
import { ImageLibraryPanel } from "../library/ImageLibraryPage";
import type { ImageLibraryItem } from "../../../core/storage/repo";
import {
  SCENE_PROMPT_APPROVAL_EVENT,
  type ScenePromptApprovalDetail,
} from "./hooks/useChatEnhancementsController";
import {
  asrCorrectionUpsert,
  asrIgnoreSuggestion,
  asrSuggestCorrectionsFromEdit,
  asrWhisperListInstalledModels,
  asrWhisperTranscribePcm,
  micConstraintsWithStoredDevice,
  type AsrInstalledWhisperModel,
  type AsrLearnedSuggestion,
} from "../../../core/asr";

type AudioHighlightRange = { start: number; end: number };

type QueuedAudioPlayback = {
  queueId: string;
  messageId: string;
  cancelled: boolean;
  activeRequestId: string | null;
  resolveCurrentChunk: (() => void) | null;
};

const LONG_PRESS_DELAY = 450;
const SCROLL_THRESHOLD = 10; // pixels of movement to cancel long press
const AUTOLOAD_TOP_THRESHOLD_PX = 120;
const STICKY_BOTTOM_THRESHOLD_PX = 80;
const MAX_AUDIO_CACHE_ENTRIES = 50;
const MOBILE_KEYBOARD_THRESHOLD_PX = 120;

function mergeFloat32Chunks(chunks: Float32Array[]) {
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const merged = new Float32Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  return merged;
}

type FooterRecorderSession = {
  stream: MediaStream;
  audioContext: AudioContext;
  source: MediaStreamAudioSourceNode;
  processor: ScriptProcessorNode;
  analyser: AnalyserNode;
  chunks: Float32Array[];
  sampleRate: number;
  startedAt: number;
};

export function ChatConversationPage() {
  const { characterId } = useParams<{ characterId: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const location = useLocation();
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
    character: layoutCharacter,
    backgroundImageData,
    backgroundImageLoading,
    isBackgroundLight,
    theme,
    chatAppearance,
    chatController,
    setDraftAppearanceOverride,
    reloadCharacter,
    appearanceFieldUpdater,
    registerAppearanceFieldUpdater,
  } = useChatLayoutContext();
  const [appearanceDrawerOpen, setAppearanceDrawerOpen] = useState(false);
  const viewportWidth = useViewportWidth();
  const widgetLayout = getChatWidgetLayout(chatAppearance, viewportWidth);
  const widgetsOn = widgetLayout.enabled;
  const [widgetEditNonce, setWidgetEditNonce] = useState(0);
  const requestWidgetEdit = widgetsOn ? () => setWidgetEditNonce((n) => n + 1) : undefined;
  const headerInside = widgetsOn && chatAppearance.chatHeaderMoves;
  const footerInside = widgetsOn && chatAppearance.chatFooterMoves;
  const applyHeaderColumnClass = !widgetsOn && chatAppearance.chatHeaderMoves;
  const applyFooterColumnClass = !widgetsOn && chatAppearance.chatFooterMoves;
  const widgetLeftNodes = widgetLayout.showLeft
    ? widgetLayout.showRight
      ? chatAppearance.chatWidgetSlots.left
      : [...chatAppearance.chatWidgetSlots.left, ...chatAppearance.chatWidgetSlots.right]
    : [];
  const widgetRightNodes = widgetLayout.showRight
    ? widgetLayout.showLeft
      ? chatAppearance.chatWidgetSlots.right
      : [...chatAppearance.chatWidgetSlots.right, ...chatAppearance.chatWidgetSlots.left]
    : [];

  const scrollContainerRef = useRef<HTMLElement | null>(null);
  const footerRef = useRef<HTMLDivElement | null>(null);
  const [footerHeight, setFooterHeight] = useState(0);
  useEffect(() => {
    const el = footerRef.current;
    if (!el) return;
    const update = () => setFooterHeight(el.offsetHeight);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  });
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
  const [supportsAudioInput, setSupportsAudioInput] = useState(false);
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
  const [audioHighlightByMessage, setAudioHighlightByMessage] = useState<
    Record<string, AudioHighlightRange>
  >({});
  const audioPlaybackRef = useRef<HTMLAudioElement | null>(null);
  const audioPlayingMessageIdRef = useRef<string | null>(null);
  const audioRequestRef = useRef<{ requestId: string; messageId: string } | null>(null);
  const audioQueueRef = useRef<QueuedAudioPlayback | null>(null);
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
  const [showTimeOverrideMenu, setShowTimeOverrideMenu] = useState(false);
  const [showBranchNavMenu, setShowBranchNavMenu] = useState(false);
  const [childForks, setChildForks] = useState<
    Map<string, Array<{ id: string; title: string; characterId: string }>>
  >(new Map());
  const [branchPickerMessageId, setBranchPickerMessageId] = useState<string | null>(null);
  const [showDiceMenu, setShowDiceMenu] = useState(false);
  const [diceNotation, setDiceNotation] = useState("1d20");
  const [diceEditing, setDiceEditing] = useState(false);
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
  const [shouldTriggerAudioInput, setShouldTriggerAudioInput] = useState(false);
  const [savingSessionBackground, setSavingSessionBackground] = useState(false);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const isMobile = useMemo(() => getPlatform().type === "mobile", []);
  const [settingsDrawerOpen, setSettingsDrawerOpen] = useState(false);
  const footerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const footerRecorderRef = useRef<FooterRecorderSession | null>(null);
  const footerRecordingTimerRef = useRef<number | null>(null);
  const [footerAsrMode, setFooterAsrMode] = useState<"idle" | "recording" | "transcribing">("idle");
  const [footerRecordingMs, setFooterRecordingMs] = useState(0);
  const [footerAnalyser, setFooterAnalyser] = useState<AnalyserNode | null>(null);
  const [footerAsrBusy, setFooterAsrBusy] = useState(false);
  const [footerAsrRawText, setFooterAsrRawText] = useState("");
  const [footerAsrBaseText, setFooterAsrBaseText] = useState("");
  const [footerAsrSuggestions, setFooterAsrSuggestions] = useState<AsrLearnedSuggestion[]>([]);
  const [footerAsrLearning, setFooterAsrLearning] = useState(false);
  const [installedWhisperModels, setInstalledWhisperModels] = useState<AsrInstalledWhisperModel[]>([]);
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

  const branchedFromMessageId = chatController.session?.branchedFromMessageId ?? null;
  const parentSessionId = chatController.session?.parentSessionId ?? null;
  const [parentBranchMeta, setParentBranchMeta] = useState<{
    title: string;
    characterId: string;
  } | null>(null);
  useEffect(() => {
    let cancelled = false;
    if (!parentSessionId || !branchedFromMessageId) {
      setParentBranchMeta(null);
      return;
    }
    void getSessionMeta(parentSessionId)
      .then((parent) => {
        if (cancelled) return;
        setParentBranchMeta(
          parent ? { title: parent.title, characterId: parent.characterId } : null,
        );
      })
      .catch(() => {
        if (!cancelled) setParentBranchMeta(null);
      });
    return () => {
      cancelled = true;
    };
  }, [parentSessionId, branchedFromMessageId]);

  useEffect(() => {
    void asrWhisperListInstalledModels()
      .then(setInstalledWhisperModels)
      .catch((error) => {
        console.error("Failed to load installed Whisper models for chat footer:", error);
      });
  }, []);

  useEffect(() => {
    return () => {
      const session = footerRecorderRef.current;
      if (session) {
        session.processor.disconnect();
        session.source.disconnect();
        session.stream.getTracks().forEach((track) => track.stop());
        void session.audioContext.close();
        footerRecorderRef.current = null;
      }
      if (footerRecordingTimerRef.current != null) {
        window.clearInterval(footerRecordingTimerRef.current);
        footerRecordingTimerRef.current = null;
      }
    };
  }, []);

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
    handleSendSystemMessage,
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
    scenePromptStreaming,
    generateAiScenePrompt,
    applySceneImagePrompt,
  } = chatController;

  useEffect(() => {
    let cancelled = false;
    if (!sessionId || messages.length === 0) {
      setChildForks(new Map());
      return;
    }
    const parentMessages = messages;
    void (async () => {
      try {
        const tree = await listBranchTree(sessionId);
        const children = tree.filter((preview) => preview.parentSessionId === sessionId);
        if (children.length === 0) {
          if (!cancelled) setChildForks(new Map());
          return;
        }
        const map = new Map<string, Array<{ id: string; title: string; characterId: string }>>();
        for (const child of children) {
          const full = await getSession(child.id);
          if (!full?.branchedFromMessageId) continue;
          const childForkIdx = full.messages.findIndex(
            (message) => message.id === full.branchedFromMessageId,
          );
          if (childForkIdx < 0) continue;
          const childForkMessage = full.messages[childForkIdx];
          let forkMessage: (typeof parentMessages)[number] | undefined =
            parentMessages[childForkIdx];
          if (
            !forkMessage ||
            forkMessage.content !== childForkMessage.content ||
            forkMessage.role !== childForkMessage.role
          ) {
            forkMessage = parentMessages.find(
              (message) =>
                message.role === childForkMessage.role &&
                message.content === childForkMessage.content,
            );
          }
          if (!forkMessage) continue;
          const list = map.get(forkMessage.id) ?? [];
          list.push({ id: child.id, title: child.title, characterId: child.characterId });
          map.set(forkMessage.id, list);
        }
        if (!cancelled) setChildForks(map);
      } catch {
        if (!cancelled) setChildForks(new Map());
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [sessionId, messages.length]);

  const [widgetPersonas, setWidgetPersonas] = useState<Persona[]>([]);
  const [widgetModels, setWidgetModels] = useState<Model[]>([]);
  const resolveModelName = useCallback(
    (modelId?: string | null): string | undefined => {
      if (!modelId) return undefined;
      return widgetModels.find((m) => m.id === modelId)?.displayName ?? modelId;
    },
    [widgetModels],
  );
  useEffect(() => {
    let cancelled = false;
    void Promise.all([listPersonas(), readSettings()]).then(([ps, s]) => {
      if (cancelled) return;
      setWidgetPersonas(ps);
      setWidgetModels(s.models ?? []);
    });
    return () => {
      cancelled = true;
    };
  }, [character?.id]);

  const widgetCtxValue: WidgetActionContext = useMemo(() => {
    const lastAssistantMessage = [...messages]
      .reverse()
      .find((m) => m.role === "assistant" && !m.id.startsWith("placeholder"));
    return {
      character,
      persona: chatController.persona,
      session: chatController.session,
      hasBackground: !!backgroundImageData,
      messageCount: messages.filter((m) => !m.id.startsWith("placeholder")).length,
      sceneName: selectedScene?.direction?.trim() || (selectedScene ? t("chats.message.sceneLabel") : null),
      memories: chatController.session?.memories ?? [],
      personas: widgetPersonas,
      models: widgetModels,
      currentModelId: character?.defaultModelId ?? null,
      swapPlacesActive: swapPlaces,
      voiceAutoplayActive:
        chatController.session?.voiceAutoplay ?? character?.voiceAutoplay ?? false,
      canRegenerate: !!lastAssistantMessage && !chatController.sending,
      canContinue: messages.length > 0 && !chatController.sending,
      isGenerating: chatController.sending,
      onSelectPersona: async (personaId) => {
        const current = chatController.session;
        if (!current) return;
        await saveSession({
          ...current,
          personaId: personaId ?? null,
          updatedAt: Date.now(),
        });
      },
      onSelectModel: async (modelId) => {
        if (!character) return;
        await saveCharacter({ id: character.id, defaultModelId: modelId ?? null });
        reloadCharacter();
      },
      onAuthorNoteSaved: (next) => {
        if (next) setSessionForHeader(next);
      },
      onRegenerate: async () => {
        if (lastAssistantMessage) {
          await chatController.handleRegenerate(lastAssistantMessage, { swapPlaces });
        }
      },
      onToggleSwapPlaces: () => setSwapPlaces((s) => !s),
      onNewSession: () => {
        if (!characterId) return;
        navigate(`/chat/${characterId}`);
      },
      onContinue: async () => {
        await chatController.handleContinue({ swapPlaces });
      },
      onAbort: async () => {
        await chatController.handleAbort();
      },
      onViewHistory: () => {
        if (!characterId) return;
        navigate(`/chat/${characterId}/history`);
      },
      onOpenMemories: () => {
        if (!characterId) return;
        const sid = chatController.session?.id;
        navigate(
          character?.mode === "companion"
            ? Routes.chatCompanionMemories(characterId, sid)
            : Routes.chatMemories(characterId, sid),
        );
      },
      onOpenSearch: () => {
        if (!characterId) return;
        navigate(Routes.chatSearch(characterId, chatController.session?.id));
      },
      onToggleVoiceAutoplay: async () => {
        const current = chatController.session;
        if (!current) return;
        const active = current.voiceAutoplay ?? character?.voiceAutoplay ?? false;
        const next = { ...current, voiceAutoplay: !active, updatedAt: Date.now() };
        await saveSession(next);
        setSessionForHeader(next);
      },
      onUpdateScratchPad: async (nodeId, content) => {
        const target = character;
        if (!target) return;
        const fresh = await getCharacter(target.id);
        const existing = (fresh?.chatAppearance ?? target.chatAppearance ?? {}) as Record<
          string,
          unknown
        >;
        const slots = (existing.chatWidgetSlots as WidgetSlots | undefined) ?? {
          left: [],
          right: [],
        };
        await updateCharacterChatAppearance(target.id, {
          ...existing,
          chatWidgetSlots: {
            left: setScratchPadContentOnNode(slots.left, nodeId, content),
            right: setScratchPadContentOnNode(slots.right, nodeId, content),
          },
        });
        reloadCharacter();
      },
      onUpdateAuthorNote: async (content) => {
        const current = chatController.session;
        if (!current) return;
        const trimmed = content.trim();
        const nextNote = trimmed.length > 0 ? trimmed : null;
        await updateSessionAuthorNote(current.id, nextNote);
        const next = { ...current, authorNote: nextNote, updatedAt: Date.now() };
        setSessionForHeader(next);
      },
      onUpdateNode: async (nodeId, patch) => {
        const target = character;
        if (!target) return;
        const fresh = await getCharacter(target.id);
        const existing = (fresh?.chatAppearance ?? target.chatAppearance ?? {}) as Record<
          string,
          unknown
        >;
        const slots = (existing.chatWidgetSlots as WidgetSlots | undefined) ?? {
          left: [],
          right: [],
        };
        await updateCharacterChatAppearance(target.id, {
          ...existing,
          chatWidgetSlots: {
            left: patchWidgetNode(slots.left, nodeId, patch as Partial<WidgetNode>),
            right: patchWidgetNode(slots.right, nodeId, patch as Partial<WidgetNode>),
          },
        });
        reloadCharacter();
      },
      onUpdateCompanionTimeOverride: async (override: CompanionTimeOverride | null) => {
        const current = chatController.session;
        if (!current) return;
        const nextCompanionState = CompanionSessionStateSchema.parse({
          ...(current.companionState ?? {}),
          preferences: {
            ...(current.companionState?.preferences ?? {}),
            timeOverride: override ?? undefined,
          },
          updatedAt: Date.now(),
        });
        const next = {
          ...current,
          companionState: nextCompanionState,
          updatedAt: Date.now(),
        };
        await saveSession(next);
        setSessionForHeader(next);
      },
      onInsertText: (text) => {
        const trimmed = draft.trimEnd();
        setDraft(trimmed ? `${trimmed} ${text}` : text);
      },
    };
  }, [
    character,
    chatController,
    widgetPersonas,
    widgetModels,
    swapPlaces,
    messages,
    characterId,
    navigate,
    reloadCharacter,
    backgroundImageData,
    selectedScene,
    setDraft,
    draft,
  ]);

  const persistWidgetSlots = useCallback(
    async (slots: WidgetSlots) => {
      const target = layoutCharacter ?? character;
      if (!target) return;
      const fresh = await getCharacter(target.id);
      const existing = (fresh?.chatAppearance ?? target.chatAppearance ?? {}) as Record<
        string,
        unknown
      >;
      await updateCharacterChatAppearance(target.id, {
        ...existing,
        chatWidgetSlots: slots,
      });
      reloadCharacter();
    },
    [layoutCharacter, character, reloadCharacter],
  );

  const persistColumnWidthPx = useCallback(
    async (px: number) => {
      const target = layoutCharacter ?? character;
      if (!target) return;
      const fresh = await getCharacter(target.id);
      const existing = (fresh?.chatAppearance ?? target.chatAppearance ?? {}) as Record<
        string,
        unknown
      >;
      await updateCharacterChatAppearance(target.id, {
        ...existing,
        chatColumnWidth: "custom",
        chatColumnWidthPx: px,
      });
      reloadCharacter();
    },
    [layoutCharacter, character, reloadCharacter],
  );

  const returnPathRef = useRef(`${location.pathname}${location.search}`);
  returnPathRef.current = `${location.pathname}${location.search}`;
  const widgetImageSelectionKey = characterId ? `widget-image:${characterId}` : null;
  const widgetImageHandledRef = useRef(false);
  const [widgetEditRestore, setWidgetEditRestore] = useState<WidgetEditRestore | null>(null);
  const beginLibraryImagePick = useCallback(
    (nodeId: string) => {
      if (!widgetImageSelectionKey) return;
      sessionStorage.setItem(
        `widget-image-target:${widgetImageSelectionKey}`,
        JSON.stringify({ nodeId }),
      );
      navigate("/library/images/pick", {
        state: {
          returnPath: returnPathRef.current,
          selectionStorageKey: widgetImageSelectionKey,
          selectionKind: "avatar",
        },
      });
    },
    [navigate, widgetImageSelectionKey],
  );

  useEffect(() => {
    const target = layoutCharacter ?? character;
    if (!target || !widgetImageSelectionKey) return;
    if (widgetImageHandledRef.current) return;
    const selectionKey = buildAvatarLibrarySelectionKey(widgetImageSelectionKey);
    const targetKey = `widget-image-target:${widgetImageSelectionKey}`;
    const rawSelection = sessionStorage.getItem(selectionKey);
    const rawTarget = sessionStorage.getItem(targetKey);
    if (!rawSelection || !rawTarget) return;

    let filePath: string | undefined;
    let nodeId: string | undefined;
    try {
      filePath = (JSON.parse(rawSelection) as AvatarLibrarySelectionPayload).filePath;
      nodeId = (JSON.parse(rawTarget) as { nodeId: string }).nodeId;
    } catch (err) {
      console.error("Failed to parse widget library selection:", err);
    }
    widgetImageHandledRef.current = true;
    sessionStorage.removeItem(selectionKey);
    sessionStorage.removeItem(targetKey);
    if (!filePath || !nodeId) return;
    const resolvedNodeId = nodeId;

    void (async () => {
      const dataUrl = await convertFilePathToDataUrl(filePath!);
      if (!dataUrl) return;
      const imageId = await convertToImageRef(dataUrl);
      if (!imageId) return;
      const fresh = await getCharacter(target.id);
      const existing = (fresh?.chatAppearance ?? target.chatAppearance ?? {}) as Record<
        string,
        unknown
      >;
      const slots = (existing.chatWidgetSlots as WidgetSlots | undefined) ?? {
        left: [],
        right: [],
      };
      const nextSlots: WidgetSlots = {
        left: setLibraryImageOnNode(slots.left, resolvedNodeId, imageId),
        right: setLibraryImageOnNode(slots.right, resolvedNodeId, imageId),
      };
      await updateCharacterChatAppearance(target.id, {
        ...existing,
        chatWidgetSlots: nextSlots,
      });
      reloadCharacter();
      setWidgetEditRestore({ slots: nextSlots, openNodeId: resolvedNodeId });
    })();
  }, [widgetImageSelectionKey, layoutCharacter, character, reloadCharacter]);

  const beetrootRain = useBeetrootRain();
  useBeetrootEasterEgg({ messages, fire: beetrootRain.fire });

  useEffect(() => {
    if (!footerAsrRawText.trim() || !draft.trim() || draft.trim() === footerAsrBaseText.trim()) {
      setFooterAsrSuggestions([]);
      return;
    }

    const timeoutId = window.setTimeout(() => {
      void asrSuggestCorrectionsFromEdit({
        before: footerAsrRawText,
        after: draft,
        language: null,
        scope: "conversation",
      })
        .then((next) => {
          setFooterAsrSuggestions((prev) => {
            const seen = new Set<string>();
            const merged: AsrLearnedSuggestion[] = [];
            const key = (s: AsrLearnedSuggestion) =>
              `${s.normalizedWrong}::${s.normalizedCorrect}`;
            for (const s of prev) {
              if (next.some((n) => key(n) === key(s))) {
                seen.add(key(s));
                merged.push(s);
              }
            }
            for (const s of next) {
              if (!seen.has(key(s))) {
                seen.add(key(s));
                merged.push(s);
              }
            }
            return merged;
          });
        })
        .catch((error) => {
          console.error("Failed to suggest ASR corrections from footer edit:", error);
        });
    }, 250);

    return () => window.clearTimeout(timeoutId);
  }, [draft, footerAsrRawText, footerAsrBaseText]);

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
        throw new Error(t("chats.errors.loadSourceSessionFailed"));
      }

      if (!sourceSession.messages.some((msg) => msg.id === messageToBranch.id)) {
        throw new Error(t("chats.errors.messageNotInSession"));
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

  const visibleMessages = useMemo(
    () => messages.filter((message) => message.role !== "system" || message.visibleInChat),
    [messages],
  );
  const isGenerating = sending || regeneratingMessageId !== null;
  const lastMessageContentLength = visibleMessages[visibleMessages.length - 1]?.content.length ?? 0;

  useEffect(() => {
    const checkModelCapabilities = async () => {
      if (!character) {
        setSupportsImageInput(false);
        setSupportsAudioInput(false);
        return;
      }
      try {
        const settings = await readSettings();
        const effectiveModelId = character.defaultModelId || settings.defaultModelId;
        const currentModel = settings.models.find((m: Model) => m.id === effectiveModelId);
        const hasImageScope = currentModel?.inputScopes?.includes("image") ?? false;
        const hasAudioScope = currentModel?.inputScopes?.includes("audio") ?? false;
        setSupportsImageInput(hasImageScope);
        setSupportsAudioInput(hasAudioScope);
      } catch (err) {
        console.error("Failed to check model capabilities:", err);
        setSupportsImageInput(false);
        setSupportsAudioInput(false);
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

  const setAudioHighlight = useCallback(
    (messageId: string, range: AudioHighlightRange | null) => {
      setAudioHighlightByMessage((prev) => {
        if (range === null) {
          if (!(messageId in prev)) return prev;
          const next = { ...prev };
          delete next[messageId];
          return next;
        }
        const current = prev[messageId];
        if (current?.start === range.start && current.end === range.end) return prev;
        return { ...prev, [messageId]: range };
      });
    },
    [],
  );

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

  const startQueuedOpenAiAudioPlayback = useCallback(
    async (params: {
      messageId: string;
      providerId: string;
      modelId: string;
      voiceId: string;
      text: string;
      prompt?: string;
      baseCacheKey: string;
    }) => {
      const chunks = splitTextForTtsQueue(params.text);
      if (chunks.length === 0) return;

      const queueId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      const queue: QueuedAudioPlayback = {
        queueId,
        messageId: params.messageId,
        cancelled: false,
        activeRequestId: null,
        resolveCurrentChunk: null,
      };
      audioQueueRef.current = queue;
      audioRequestRef.current = null;
      setAudioStatus(params.messageId, "loading");
      setAudioHighlight(params.messageId, null);

      const ensureActive = () => {
        if (queue.cancelled || audioQueueRef.current?.queueId !== queueId) {
          throw new Error("Request aborted by user");
        }
      };

      const getChunkResponse = async (chunkIndex: number): Promise<TtsPreviewResponse> => {
        ensureActive();
        const chunk = chunks[chunkIndex];
        const chunkCacheKey = `${params.baseCacheKey}::openai-tts-chunk::${chunk.index}:${chunk.start}:${chunk.end}`;
        const cached = audioPreviewCacheRef.current.get(chunkCacheKey);
        if (cached) return cached;

        const requestId = `${queueId}-${chunk.index}`;
        queue.activeRequestId = requestId;
        audioRequestRef.current = { requestId, messageId: params.messageId };
        try {
          const response = await generateTtsPreview(
            params.providerId,
            params.modelId,
            params.voiceId,
            chunk.text,
            params.prompt,
            requestId,
          );
          ensureActive();
          cacheAudioPreview(chunkCacheKey, response);
          return response;
        } finally {
          if (queue.activeRequestId === requestId) {
            queue.activeRequestId = null;
          }
          if (audioRequestRef.current?.requestId === requestId) {
            audioRequestRef.current = null;
          }
        }
      };

      const queueChunkResponse = (chunkIndex: number): Promise<TtsPreviewResponse> => {
        const promise = getChunkResponse(chunkIndex);
        void promise.catch(() => undefined);
        return promise;
      };

      const playChunk = (chunkIndex: number, response: TtsPreviewResponse) =>
        new Promise<void>((resolve) => {
          ensureActive();
          const chunk = chunks[chunkIndex];
          setAudioHighlight(params.messageId, { start: chunk.start, end: chunk.end });
          setAudioStatus(params.messageId, "playing");
          const audio = playAudioFromBase64(response.audioBase64, response.format);
          audioPlaybackRef.current = audio;
          audioPlayingMessageIdRef.current = params.messageId;

          let done = false;
          const finish = () => {
            if (done) return;
            done = true;
            if (queue.resolveCurrentChunk === finish) {
              queue.resolveCurrentChunk = null;
            }
            if (audioPlaybackRef.current === audio) {
              audioPlaybackRef.current = null;
              audioPlayingMessageIdRef.current = null;
            }
            resolve();
          };

          queue.resolveCurrentChunk = finish;
          audio.onended = finish;
          audio.onerror = finish;
        });

      try {
        let nextResponsePromise: Promise<TtsPreviewResponse> | null = queueChunkResponse(0);
        for (let index = 0; index < chunks.length; index += 1) {
          ensureActive();
          if (!nextResponsePromise) break;
          const response = await nextResponsePromise;
          ensureActive();
          nextResponsePromise = index + 1 < chunks.length ? queueChunkResponse(index + 1) : null;
          await playChunk(index, response);
          ensureActive();
          if (nextResponsePromise) {
            setAudioStatus(params.messageId, "loading");
          }
        }
      } finally {
        if (audioQueueRef.current?.queueId === queueId) {
          audioQueueRef.current = null;
          setAudioHighlight(params.messageId, null);
          setAudioStatus(params.messageId, null);
          const activeRequest = audioRequestRef.current as
            | { requestId: string; messageId: string }
            | null;
          if (activeRequest?.requestId.startsWith(queueId)) {
            audioRequestRef.current = null;
          }
        }
      }
    },
    [cacheAudioPreview, setAudioHighlight, setAudioStatus],
  );

  const startAudioPlayback = useCallback(
    (messageId: string, response: TtsPreviewResponse) => {
      setAudioHighlight(messageId, null);
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
    [setAudioHighlight, setAudioStatus],
  );

  const stopAudioPlayback = useCallback(() => {
    const queue = audioQueueRef.current;
    if (queue) {
      queue.cancelled = true;
      if (queue.activeRequestId) {
        cancelledAudioRequestsRef.current.add(queue.activeRequestId);
        void abortAudioPreview(queue.activeRequestId).catch((error) => {
          console.warn("Failed to cancel queued audio chunk:", error);
        });
      }
      queue.resolveCurrentChunk?.();
      setAudioHighlight(queue.messageId, null);
      setAudioStatus(queue.messageId, null);
      audioQueueRef.current = null;
    }

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
      setAudioHighlight(messageId, null);
      setAudioStatus(messageId, null);
    }
  }, [setAudioHighlight, setAudioStatus]);

  const cancelAudioGeneration = useCallback(async () => {
    const pending = audioRequestRef.current;
    const queue = audioQueueRef.current;
    if (!pending && !queue) return;

    if (queue) {
      queue.cancelled = true;
      queue.resolveCurrentChunk?.();
      const audio = audioPlaybackRef.current;
      if (audio) {
        audio.pause();
        audio.currentTime = 0;
        audio.onended = null;
        audio.onerror = null;
      }
      audioPlaybackRef.current = null;
      audioPlayingMessageIdRef.current = null;
      setAudioHighlight(queue.messageId, null);
      setAudioStatus(queue.messageId, null);
      audioQueueRef.current = null;
    }

    if (!pending) return;
    audioRequestRef.current = null;
    cancelledAudioRequestsRef.current.add(pending.requestId);
    setAudioHighlight(pending.messageId, null);
    setAudioStatus(pending.messageId, null);
    try {
      await abortAudioPreview(pending.requestId);
    } catch (error) {
      console.warn("Failed to cancel audio preview:", error);
    }
  }, [setAudioHighlight, setAudioStatus]);

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
          throw new Error(t("chats.errors.assignedVoiceNotFound"));
        }
        const provider = providers.find((p) => p.id === voice.providerId);
        if (!provider) {
          throw new Error(t("chats.errors.assignedProviderNotFound"));
        }

        const cacheKey = buildAudioCacheKey({
          providerId: voice.providerId,
          modelId: voice.modelId,
          voiceId: voice.voiceId,
          text: trimmedText,
          prompt: voice.prompt,
        });

        if (provider.providerType === "openai_tts") {
          try {
            await startQueuedOpenAiAudioPlayback({
              messageId: message.id,
              providerId: voice.providerId,
              modelId: voice.modelId,
              voiceId: voice.voiceId,
              text: trimmedText,
              prompt: voice.prompt,
              baseCacheKey: cacheKey,
            });
          } catch (error) {
            if (audioRequestRef.current?.requestId === requestId) {
              audioRequestRef.current = null;
            }
            setAudioHighlight(message.id, null);
            setAudioStatus(message.id, null);
            const messageText = error instanceof Error ? error.message : String(error);
            const isAbort =
              messageText.toLowerCase().includes("aborted") ||
              messageText.toLowerCase().includes("cancel");
            if (isAbort) return;
            throw error;
          }
          return;
        }

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
          throw new Error(t("chats.errors.voiceMissingProviderDetails"));
        }
        const provider = providers.find((p) => p.id === providerId);
        if (!provider) {
          throw new Error(t("chats.errors.assignedProviderNotFound"));
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
          throw new Error(t("chats.errors.noAudioModelsForProvider"));
        }

        const cacheKey = buildAudioCacheKey({
          providerId,
          modelId,
          voiceId,
          text: trimmedText,
        });

        if (provider.providerType === "openai_tts") {
          try {
            await startQueuedOpenAiAudioPlayback({
              messageId: message.id,
              providerId,
              modelId,
              voiceId,
              text: trimmedText,
              baseCacheKey: cacheKey,
            });
          } catch (error) {
            if (audioRequestRef.current?.requestId === requestId) {
              audioRequestRef.current = null;
            }
            setAudioHighlight(message.id, null);
            setAudioStatus(message.id, null);
            const messageText = error instanceof Error ? error.message : String(error);
            const isAbort =
              messageText.toLowerCase().includes("aborted") ||
              messageText.toLowerCase().includes("cancel");
            if (isAbort) return;
            throw error;
          }
          return;
        }

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
      setAudioHighlight,
      setAudioStatus,
      startAudioPlayback,
      startQueuedOpenAiAudioPlayback,
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
                t("chats.errors.helpMeReplyFailed");
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
            setHelpMeReplyError(t("chats.errors.helpMeReplyNoReply"));
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

  const handlePlusMenuAudioUpload = useCallback(() => {
    setShowPlusMenu(false);
    setShouldTriggerAudioInput(true);
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

  const handleApplyCompanionTimeOverride = useCallback(
    async (override: CompanionTimeOverride | null) => {
      const current = chatController.session;
      if (!current) return;
      const nextCompanionState = CompanionSessionStateSchema.parse({
        ...(current.companionState ?? {}),
        preferences: {
          ...(current.companionState?.preferences ?? {}),
          timeOverride: override ?? undefined,
        },
        updatedAt: Date.now(),
      });
      const next = {
        ...current,
        companionState: nextCompanionState,
        updatedAt: Date.now(),
      };
      await saveSession(next);
      setSessionForHeader(next);
    },
    [chatController, saveSession],
  );

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

  const stopFooterRecordingVisuals = useCallback(() => {
    if (footerRecordingTimerRef.current != null) {
      window.clearInterval(footerRecordingTimerRef.current);
      footerRecordingTimerRef.current = null;
    }
    setFooterRecordingMs(0);
    setFooterAnalyser(null);
  }, []);

  const stopFooterRecording = useCallback(async () => {
    const session = footerRecorderRef.current;
    if (!session) return;

    footerRecorderRef.current = null;
    setFooterAsrMode("transcribing");
    setFooterAsrBusy(true);

    session.processor.disconnect();
    session.source.disconnect();
    session.stream.getTracks().forEach((track) => track.stop());
    await session.audioContext.close();

    try {
      const merged = mergeFloat32Chunks(session.chunks);
      if (merged.length === 0) {
        stopFooterRecordingVisuals();
        setFooterAsrMode("idle");
        setFooterAsrBusy(false);
        setError(t("chats.asr.noAudioCaptured"));
        return;
      }

      const modelPath = installedWhisperModels[0]?.path;
      if (!modelPath) {
        stopFooterRecordingVisuals();
        setFooterAsrMode("idle");
        setFooterAsrBusy(false);
        setError(t("chats.asr.noModelInstalled"));
        return;
      }

      const pcmBytes = new Uint8Array(merged.buffer, merged.byteOffset, merged.byteLength);
      const result = await asrWhisperTranscribePcm({
        modelPath,
        pcmBytes,
        sampleRateHz: session.sampleRate,
        channels: 1,
        scopes: ["conversation", "global"],
        useGpu: true,
        forceCpu: false,
        keepModelLoaded: true,
      });

      const nextDraft = result.correctedText?.trim() || result.rawText?.trim();
      setFooterAsrRawText(result.rawText || "");
      setFooterAsrBaseText(nextDraft || "");
      setDraft(nextDraft || "");
      setFooterAsrSuggestions([]);
      setFooterAsrMode("idle");
    } catch (error) {
      console.error("Failed to transcribe footer recording:", error);
      setFooterAsrMode("idle");
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      stopFooterRecordingVisuals();
      setFooterAsrBusy(false);
    }
  }, [installedWhisperModels, setDraft, setError, stopFooterRecordingVisuals]);

  const handleFooterMicClick = useCallback(async () => {
    if (sending || footerAsrBusy) return;

    if (footerAsrMode === "recording") {
      await stopFooterRecording();
      return;
    }

    setError(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: micConstraintsWithStoredDevice({
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        }),
      });
      const audioContext = new AudioContext();
      const source = audioContext.createMediaStreamSource(stream);
      const processor = audioContext.createScriptProcessor(4096, 1, 1);
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 1024;
      analyser.smoothingTimeConstant = 0.4;
      const chunks: Float32Array[] = [];
      processor.onaudioprocess = (event) => {
        chunks.push(new Float32Array(event.inputBuffer.getChannelData(0)));
      };
      source.connect(analyser);
      source.connect(processor);
      processor.connect(audioContext.destination);

      footerRecorderRef.current = {
        stream,
        audioContext,
        source,
        processor,
        analyser,
        chunks,
        sampleRate: audioContext.sampleRate,
        startedAt: Date.now(),
      };
      setFooterAnalyser(analyser);
      setFooterAsrRawText("");
      setFooterAsrBaseText("");
      setFooterAsrSuggestions([]);
      setFooterAsrMode("recording");
      setFooterRecordingMs(0);
      footerRecordingTimerRef.current = window.setInterval(() => {
        const startedAt = footerRecorderRef.current?.startedAt;
        if (startedAt) setFooterRecordingMs(Date.now() - startedAt);
      }, 200);
    } catch (error) {
      console.error("Failed to start footer recording:", error);
      setError(error instanceof Error ? error.message : String(error));
    }
  }, [footerAsrBusy, footerAsrMode, sending, setError, stopFooterRecording]);

  const handleLearnFooterSuggestion = useCallback(
    async (suggestion: AsrLearnedSuggestion) => {
      if (footerAsrLearning) return;
      setFooterAsrLearning(true);
      try {
        await asrCorrectionUpsert({
          wrong: suggestion.wrong,
          correct: suggestion.correct,
          confidence: suggestion.confidence,
          language: suggestion.language ?? null,
          scope: suggestion.scope,
          userApproved: true,
        });
        setFooterAsrSuggestions((prev) =>
          prev.filter(
            (item) =>
              !(
                item.normalizedWrong === suggestion.normalizedWrong &&
                item.normalizedCorrect === suggestion.normalizedCorrect
              ),
          ),
        );
      } catch (error) {
        console.error("Failed to learn footer ASR correction:", error);
        setError(error instanceof Error ? error.message : String(error));
      } finally {
        setFooterAsrLearning(false);
      }
    },
    [footerAsrLearning, setError],
  );

  const handleIgnoreFooterSuggestion = useCallback(
    async (suggestion: AsrLearnedSuggestion) => {
      if (footerAsrLearning) return;
      setFooterAsrLearning(true);
      try {
        await asrIgnoreSuggestion(suggestion);
        setFooterAsrSuggestions((prev) =>
          prev.filter(
            (item) =>
              !(
                item.normalizedWrong === suggestion.normalizedWrong &&
                item.normalizedCorrect === suggestion.normalizedCorrect
              ),
          ),
        );
      } catch (error) {
        console.error("Failed to ignore footer ASR suggestion:", error);
        setError(error instanceof Error ? error.message : String(error));
      } finally {
        setFooterAsrLearning(false);
      }
    },
    [footerAsrLearning, setError],
  );

  const cancelFooterRecording = useCallback(() => {
    const session = footerRecorderRef.current;
    if (!session) return;
    footerRecorderRef.current = null;
    stopFooterRecordingVisuals();
    try {
      session.processor.disconnect();
      session.source.disconnect();
      session.stream.getTracks().forEach((track) => track.stop());
      void session.audioContext.close();
    } catch (err) {
      console.warn("Failed to cleanly cancel footer recording:", err);
    }
    setFooterAsrMode("idle");
    setFooterAsrRawText("");
    setFooterAsrBaseText("");
    setFooterAsrSuggestions([]);
  }, [stopFooterRecordingVisuals]);

  const handleSendMessage = useCallback(async () => {
    if (sending) return;
    setError(null);

    const hasContent = draft.trim().length > 0 || pendingAttachments.length > 0;

    if (hasContent) {
      const content = draft.trim();
      playAccessibilitySound("send", accessibilitySettings);
      await handleSend(content, undefined, { swapPlaces });
      setFooterAsrMode("idle");
      setFooterAsrRawText("");
      setFooterAsrBaseText("");
      setFooterAsrSuggestions([]);
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
    setFooterAsrMode,
  ]);

  const handleSendVisibleSystemMessage = useCallback(async () => {
    if (sending) return;
    setError(null);
    playAccessibilitySound("send", accessibilitySettings);
    await handleSendSystemMessage(draft, undefined);
    setFooterAsrMode("idle");
    setFooterAsrRawText("");
    setFooterAsrBaseText("");
    setFooterAsrSuggestions([]);
  }, [
    accessibilitySettings,
    draft,
    handleSendSystemMessage,
    sending,
    setError,
  ]);

  const captureFooterFocusForDrawer = useCallback(() => {
    shouldRestoreFooterFocusRef.current = document.activeElement === footerTextareaRef.current;
  }, []);

  const handleRegenerateMessage = useCallback(
    async (message: StoredMessage, options?: { guidance?: string; modelId?: string }) => {
      await handleRegenerate(message, {
        swapPlaces,
        guidance: options?.guidance,
        modelId: options?.modelId,
      });
    },
    [handleRegenerate, swapPlaces],
  );

  const footerInlinePanel =
    footerAsrSuggestions.length === 0 ? undefined : (
      <div className="space-y-1.5 px-4 py-2">
        {footerAsrSuggestions.map((suggestion, idx) => (
          <div
            key={`${suggestion.normalizedWrong}-${suggestion.normalizedCorrect}-${idx}`}
            className="flex flex-wrap items-center justify-between gap-2"
          >
            <div className="min-w-0 text-xs text-fg/65">
              {t("chats.asr.learnCorrection")}{" "}
              <span className="text-danger/80 line-through decoration-danger/40">
                {suggestion.wrong}
              </span>
              <span className="mx-1.5 text-fg/40">→</span>
              <span className="font-medium text-fg">{suggestion.correct}</span>
            </div>
            <div className="flex shrink-0 gap-2">
              <button
                type="button"
                onClick={() => void handleLearnFooterSuggestion(suggestion)}
                disabled={footerAsrLearning}
                className={cn(
                  "rounded-full border border-accent/30 bg-accent/15 px-3 py-1 text-xs font-medium text-accent",
                  "hover:border-accent/50 hover:bg-accent/20 disabled:opacity-50",
                )}
              >
                {footerAsrLearning ? t("chats.asr.learning") : t("chats.asr.learn")}
              </button>
              <button
                type="button"
                onClick={() => void handleIgnoreFooterSuggestion(suggestion)}
                disabled={footerAsrLearning}
                className={cn(
                  "rounded-full border border-fg/15 bg-fg/8 px-3 py-1 text-xs font-medium text-fg/70",
                  "hover:border-fg/25 hover:bg-fg/12 disabled:opacity-50",
                )}
              >
                {t("chats.asr.ignore")}
              </button>
            </div>
          </div>
        ))}
      </div>
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
  }, [visibleMessages.length, lastMessageContentLength, isGenerating, updateIsAtBottom]);

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
    const rafIds: number[] = [];
    const timeoutIds: number[] = [];
    let highlightTimeoutId: number | null = null;

    isAtBottomRef.current = false;
    setIsAtBottom(false);

    const centerOnMessage = () => {
      const container = scrollContainerRef.current;
      const element = document.getElementById(`message-${jumpToMessageId}`);
      if (!container || !element) return false;
      const containerRect = container.getBoundingClientRect();
      const elementRect = element.getBoundingClientRect();
      const delta =
        elementRect.top -
        containerRect.top -
        Math.max(0, (container.clientHeight - element.clientHeight) / 2);
      container.scrollTop += delta;
      return true;
    };

    const run = async () => {
      await ensureMessageLoaded(jumpToMessageId);
      if (cancelled) return;

      isAtBottomRef.current = false;
      setIsAtBottom(false);

      let tries = 0;
      const attempt = () => {
        if (cancelled) return;
        const found = centerOnMessage();
        if (found && tries === 0) {
          const element = document.getElementById(`message-${jumpToMessageId}`);
          element?.classList.add(
            "bg-white/10",
            "rounded-lg",
            "transition-colors",
            "duration-1000",
          );
          highlightTimeoutId = window.setTimeout(() => {
            element?.classList.remove("bg-white/10");
          }, 2000);
        }
        tries += 1;
        if (!found && tries < 20) {
          rafIds.push(window.requestAnimationFrame(attempt));
        }
      };
      rafIds.push(window.requestAnimationFrame(attempt));

      for (const ms of [150, 450]) {
        timeoutIds.push(
          window.setTimeout(() => {
            if (!cancelled) centerOnMessage();
          }, ms),
        );
      }
    };

    void run();

    return () => {
      cancelled = true;
      rafIds.forEach((id) => window.cancelAnimationFrame(id));
      timeoutIds.forEach((id) => window.clearTimeout(id));
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
  const scrollButtonBottomOffset =
    footerHeight > 0
      ? `${footerHeight + 12}px`
      : `calc(env(safe-area-inset-bottom) + ${keyboardInset}px + 88px)`;

  return (
    <div
      className={cn(
        "relative flex h-full flex-col overflow-hidden",
        !backgroundImageData && !backgroundImageLoading && "bg-surface",
      )}
    >
      {backgroundImageData && (
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 z-0 overflow-hidden"
          style={{
            backgroundImage: `url(${backgroundImageData})`,
            backgroundSize: "cover",
            backgroundPosition: "center",
            backgroundRepeat: "no-repeat",
            filter:
              chatAppearance.backgroundBlur > 0
                ? `blur(${chatAppearance.backgroundBlur}px)`
                : undefined,
            transform: chatAppearance.backgroundBlur > 0 ? "scale(1.06)" : undefined,
          }}
        />
      )}
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

      {!headerInside && (
      <div
        className={`relative z-20 ${applyHeaderColumnClass ? getChatColumnLayout(chatAppearance).className : ""}`}
        style={applyHeaderColumnClass ? getChatColumnLayout(chatAppearance).style : undefined}
      >
        <ChatHeader
          character={character}
          persona={persona}
          swapPlaces={swapPlaces}
          sessionId={sessionId}
          session={sessionForHeader}
          hasBackgroundImage={!!backgroundImageData}
          headerOverlayClassName={theme.headerOverlay}
          transparentHeader={chatAppearance.transparentHeader}
          onSessionUpdate={handleSessionUpdate}
          onBeforeSettingsOpen={!isMobile ? captureFooterFocusForDrawer : undefined}
          onSettingsOpen={!isMobile ? () => setSettingsDrawerOpen(true) : undefined}
          onAppearanceOpen={!isMobile ? () => setAppearanceDrawerOpen(true) : undefined}
          onEditWidgets={!isMobile ? requestWidgetEdit : undefined}
        />
      </div>
      )}

      <WidgetContextProvider value={widgetCtxValue}>
      <WidgetEditProvider
        slots={chatAppearance.chatWidgetSlots}
        onPersist={persistWidgetSlots}
        onChooseLibraryImage={beginLibraryImagePick}
        restore={widgetEditRestore}
        onRestoreConsumed={() => setWidgetEditRestore(null)}
        editRequestNonce={widgetEditNonce}
      >
      <ChatWidgetArea
        widgetLayout={widgetLayout}
        leftNodes={widgetLeftNodes}
        rightNodes={widgetRightNodes}
        resizable={chatAppearance.chatColumnWidth === "custom" && appearanceDrawerOpen}
        viewportWidth={viewportWidth}
        onResizeColumn={(px) => {
          if (appearanceFieldUpdater) {
            appearanceFieldUpdater("chatColumnWidthPx", px);
          } else {
            void persistColumnWidthPx(px);
          }
        }}
      >
      {headerInside && (
      <div className="relative z-20">
        <ChatHeader
          character={character}
          persona={persona}
          swapPlaces={swapPlaces}
          sessionId={sessionId}
          session={sessionForHeader}
          hasBackgroundImage={!!backgroundImageData}
          headerOverlayClassName={theme.headerOverlay}
          transparentHeader={chatAppearance.transparentHeader}
          onSessionUpdate={handleSessionUpdate}
          onBeforeSettingsOpen={!isMobile ? captureFooterFocusForDrawer : undefined}
          onSettingsOpen={!isMobile ? () => setSettingsDrawerOpen(true) : undefined}
          onAppearanceOpen={!isMobile ? () => setAppearanceDrawerOpen(true) : undefined}
          onEditWidgets={!isMobile ? requestWidgetEdit : undefined}
        />
      </div>
      )}

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
              <span className="text-sm">{t("chats.swap.bannerActive")}</span>
              <button
                type="button"
                onClick={handleDisableSwapPlaces}
                className={cn(
                  "rounded-full border border-emerald-200/40 px-3 py-1 text-xs font-medium text-emerald-50 hover:bg-emerald-100/10",
                )}
              >
                {t("chats.swap.endSwap")}
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
          className={`${getChatColumnLayout(chatAppearance).className} ${chatAppearance.messageGap === "tight" ? "space-y-2" : chatAppearance.messageGap === "relaxed" ? "space-y-6" : "space-y-4"} px-3 pb-8 pt-4`}
          style={{
            ...getChatColumnLayout(chatAppearance).style,
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
                {t("chats.loadEarlierMessages")}
              </button>
            </div>
          )}

          <LayoutGroup id="swap-message-layout">
            {visibleMessages.map((message, index) => {
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
              const isVisibleSystem =
                renderedMessage.role === "system" && Boolean(renderedMessage.visibleInChat);
              const actionable =
                (isAssistant || isUser || isSceneMessage || isVisibleSystem) &&
                !message.id.startsWith("placeholder");
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

              const showBranchMarker =
                !!branchedFromMessageId && message.id === branchedFromMessageId;

              return (
                <Fragment key={message.id}>
                  {showBranchMarker ? (
                    <div className="my-3 flex items-center gap-2 px-2">
                      <span className="h-px flex-1 bg-fg/10" />
                      <button
                        type="button"
                        disabled={!parentBranchMeta}
                        onClick={() => {
                          if (!parentBranchMeta) return;
                          setShowBranchNavMenu(true);
                        }}
                        className={cn(
                          "inline-flex shrink-0 items-center gap-1.5 rounded-full border border-fg/10 bg-fg/4 px-3 py-1",
                          typography.caption.size,
                          "text-fg/55 transition-colors hover:bg-fg/8 hover:text-fg/80 disabled:pointer-events-none",
                        )}
                      >
                        <GitBranch size={12} />
                        {t("chats.branchTree.branchedFrom", {
                          title: parentBranchMeta?.title ?? "",
                        })}
                      </button>
                      <span className="h-px flex-1 bg-fg/10" />
                    </div>
                  ) : null}
                  <motion.div
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
                    messagesLength={visibleMessages.length}
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
                    audioHighlight={audioHighlightByMessage[message.id]}
                    onPlayAudio={handlePlayMessageAudio}
                    onStopAudio={handleStopAudio}
                    onCancelAudio={handleCancelAudio}
                    onImageClick={handleImageClick}
                    reasoning={streamingReasoning[message.id] || combinedReasoning || undefined}
                    swapPlaces={swapPlaces}
                    modelName={resolveModelName(message.modelId)}
                    models={widgetModels}
                    scenePromptStreaming={
                      scenePromptStreaming &&
                      message.role === "assistant" &&
                      index === visibleMessages.length - 1
                    }
                  />
                  </motion.div>
                  {childForks.has(message.id) ? (
                    <div className="my-3 flex items-center gap-2 px-2">
                      <span className="h-px flex-1 bg-fg/10" />
                      <button
                        type="button"
                        onClick={() => setBranchPickerMessageId(message.id)}
                        className={cn(
                          "inline-flex shrink-0 items-center gap-1.5 rounded-full border border-fg/10 bg-fg/4 px-3 py-1",
                          typography.caption.size,
                          "text-fg/55 transition-colors hover:bg-fg/8 hover:text-fg/80",
                        )}
                      >
                        <GitBranch size={12} />
                        {t("chats.branchTree.branchesFromHere", {
                          count: childForks.get(message.id)?.length ?? 0,
                        })}
                      </button>
                      <span className="h-px flex-1 bg-fg/10" />
                    </div>
                  ) : null}
                </Fragment>
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
              "absolute right-3 z-30 flex h-11 w-11 items-center justify-center",
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

      {footerInside && (
      <div
        ref={footerRef}
        className="relative z-10"
        style={{ paddingBottom: footerBottomOffset }}
      >
        <ChatFooter
          inlinePanel={footerInlinePanel}
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
          onSendSystemMessage={handleSendVisibleSystemMessage}
          onAbort={handleAbortWithFlag}
          hasBackgroundImage={!!backgroundImageData}
          footerOverlayClassName={theme.footerOverlay}
          footerOverlayColor={theme.footerOverlayColor}
          footerFgColor={theme.footerFgColor}
          footerFgMutedColor={theme.footerFgMutedColor}
          pendingAttachments={pendingAttachments}
          onAddAttachment={
            supportsImageInput || supportsAudioInput ? addPendingAttachment : undefined
          }
          onRemoveAttachment={
            supportsImageInput || supportsAudioInput ? removePendingAttachment : undefined
          }
          onOpenPlusMenu={handleOpenPlusMenu}
          onMicClick={
            installedWhisperModels.length === 0
              ? undefined
              : () => {
                  void handleFooterMicClick();
                }
          }
          micActive={footerAsrMode === "recording" || footerAsrMode === "transcribing"}
          micDisabled={footerAsrBusy}
          recordingElapsedMs={footerRecordingMs}
          recordingAnalyser={footerAnalyser}
          recordingTranscribing={footerAsrMode === "transcribing"}
          onMicCancel={cancelFooterRecording}
          composerDisabled={footerAsrMode !== "idle"}
          triggerFileInput={shouldTriggerFileInput}
          onFileInputTriggered={() => setShouldTriggerFileInput(false)}
          triggerAudioInput={shouldTriggerAudioInput}
          onAudioInputTriggered={() => setShouldTriggerAudioInput(false)}
          textareaRef={footerTextareaRef}
        />
      </div>
      )}
      </ChatWidgetArea>
      </WidgetEditProvider>
      </WidgetContextProvider>

      {!footerInside && (
      <div
        ref={footerRef}
        className={`relative z-10 ${applyFooterColumnClass ? getChatColumnLayout(chatAppearance).className : ""}`}
        style={{
          paddingBottom: footerBottomOffset,
          ...(applyFooterColumnClass ? getChatColumnLayout(chatAppearance).style : {}),
        }}
      >
        <ChatFooter
          inlinePanel={footerInlinePanel}
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
          onSendSystemMessage={handleSendVisibleSystemMessage}
          onAbort={handleAbortWithFlag}
          hasBackgroundImage={!!backgroundImageData}
          footerOverlayClassName={theme.footerOverlay}
          footerOverlayColor={theme.footerOverlayColor}
          footerFgColor={theme.footerFgColor}
          footerFgMutedColor={theme.footerFgMutedColor}
          pendingAttachments={pendingAttachments}
          onAddAttachment={
            supportsImageInput || supportsAudioInput ? addPendingAttachment : undefined
          }
          onRemoveAttachment={
            supportsImageInput || supportsAudioInput ? removePendingAttachment : undefined
          }
          onOpenPlusMenu={handleOpenPlusMenu}
          onMicClick={
            installedWhisperModels.length === 0
              ? undefined
              : () => {
                  void handleFooterMicClick();
                }
          }
          micActive={footerAsrMode === "recording" || footerAsrMode === "transcribing"}
          micDisabled={footerAsrBusy}
          recordingElapsedMs={footerRecordingMs}
          recordingAnalyser={footerAnalyser}
          recordingTranscribing={footerAsrMode === "transcribing"}
          onMicCancel={cancelFooterRecording}
          composerDisabled={footerAsrMode !== "idle"}
          triggerFileInput={shouldTriggerFileInput}
          onFileInputTriggered={() => setShouldTriggerFileInput(false)}
          triggerAudioInput={shouldTriggerAudioInput}
          onAudioInputTriggered={() => setShouldTriggerAudioInput(false)}
          textareaRef={footerTextareaRef}
        />
      </div>
      )}

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
        onOpenChatAppearance={!isMobile ? () => setAppearanceDrawerOpen(true) : undefined}
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
            {t("chats.branch.chooseCharacterPrompt")}
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
              {t("chats.branch.noOtherCharacters")}
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
            {t("chats.branch.groupOwnerLocked")}
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
            {groupBranchCreating ? t("common.buttons.creating") : t("chats.branch.createGroupBranch")}
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
          {helpMeReplyEnabled && (
            <MenuButton
              icon={Sparkles}
              title={t("chats.helpMeReply")}
              description={t("chats.helpMeReplyDesc")}
              onClick={handlePlusMenuHelpMeReply}
            />
          )}
          <MenuButton
            icon={User}
            title={t("chats.settings.persona")}
            description={persona?.title ?? t("chats.settings.noPersona")}
            onClick={() => {
              void handleOpenPersonaSelector();
            }}
          />
          {supportsImageInput && (
            <MenuButton
              icon={Image}
              title={t("chats.uploadImage")}
              description={t("chats.uploadImageDesc")}
              onClick={handlePlusMenuImageUpload}
            />
          )}
          {supportsAudioInput && (
            <MenuButton
              icon={FileAudio}
              title={t("chats.uploadAudio")}
              description={t("chats.uploadAudioDesc")}
              onClick={handlePlusMenuAudioUpload}
            />
          )}
          <MenuButton
            icon={Image}
            title={t("chats.chatBackground")}
              description={
              hasSessionBackgroundOverride
                ? t("chats.settings.sessionOverrideActive")
                : hasSceneBackgroundDefault
                  ? t("chats.background.usingScene")
                : hasCharacterBackgroundDefault
                  ? t("chats.background.usingCharacterDefault")
                  : t("chats.background.noneSelected")
            }
            onClick={() => {
              setShowPlusMenu(false);
              setShowBackgroundMenu(true);
            }}
          />
          <MenuButton
            icon={NotebookPen}
            title={t("chats.authorNote.title")}
            description={
              (sessionForHeader?.authorNote ?? chatController.session?.authorNote)?.trim()
                ? t("chats.plusMenu.authorNoteActive")
                : t("chats.plusMenu.authorNoteInactive")
            }
            onClick={handleOpenAuthorNoteMenu}
          />
          <MenuButton
            icon={Dices}
            title={t("chats.widgets.dice.defaultTitle")}
            description={t("chats.widgets.dice.menuDescription")}
            onClick={() => {
              setShowPlusMenu(false);
              setShowDiceMenu(true);
            }}
          />
          <MenuButton
            icon={ArrowLeftRight}
            title={swapPlaces ? t("chats.swapPlacesOn") : t("chats.swapPlaces")}
            description={
              swapPlaces
                ? t("chats.swap.activeDesc")
                : t("chats.swap.inactiveDesc")
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
          {(session?.mode ?? character?.mode) === "companion" &&
            (session?.companionState?.preferences?.timeAwarenessEnabled ?? false) && (
              <MenuButton
                icon={History}
                title={t("chats.timeOverride.title")}
                description={
                  session?.companionState?.preferences?.timeOverride?.mode === "frozen"
                    ? t("chats.timeOverride.badgeFrozen")
                    : session?.companionState?.preferences?.timeOverride
                      ? t("chats.timeOverride.badgeCustom")
                      : t("chats.timeOverride.badgeLive")
                }
                onClick={() => {
                  setShowPlusMenu(false);
                  setShowTimeOverrideMenu(true);
                }}
              />
            )}
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showTimeOverrideMenu}
        onClose={() => setShowTimeOverrideMenu(false)}
        title={t("chats.timeOverride.title")}
      >
        <CompanionTimeOverrideCard
          session={sessionForHeader ?? session ?? null}
          onApply={handleApplyCompanionTimeOverride}
        />
      </BottomMenu>

      <BottomMenu
        isOpen={showBranchNavMenu}
        onClose={() => setShowBranchNavMenu(false)}
        title={t("chats.branchTree.goToParentTitle")}
      >
        <div className="space-y-2">
          {parentBranchMeta ? (
            <p className="px-1 pb-1 text-xs text-fg/55">
              {t("chats.branchTree.goToParentDesc", { title: parentBranchMeta.title })}
            </p>
          ) : null}
          <MenuButton
            icon={GitBranch}
            title={t("chats.branchTree.goToParentConfirm", {
              title: parentBranchMeta?.title ?? "",
            })}
            onClick={() => {
              if (!parentBranchMeta || !parentSessionId) return;
              setShowBranchNavMenu(false);
              navigate(Routes.chatSession(parentBranchMeta.characterId, parentSessionId));
            }}
          />
          <MenuButton
            icon={X}
            title={t("chats.branchTree.goToParentCancel")}
            onClick={() => setShowBranchNavMenu(false)}
          />
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={branchPickerMessageId !== null}
        onClose={() => setBranchPickerMessageId(null)}
        title={t("chats.branchTree.branchesFromHereTitle")}
      >
        <div className="space-y-2">
          {(branchPickerMessageId ? childForks.get(branchPickerMessageId) : undefined)?.map(
            (child) => (
              <MenuButton
                key={child.id}
                icon={GitBranch}
                title={t("chats.branchTree.goToParentConfirm", { title: child.title })}
                onClick={() => {
                  setBranchPickerMessageId(null);
                  navigate(Routes.chatSession(child.characterId, child.id));
                }}
              />
            ),
          )}
          <MenuButton
            icon={X}
            title={t("chats.branchTree.goToParentCancel")}
            onClick={() => setBranchPickerMessageId(null)}
          />
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showDiceMenu}
        onClose={() => {
          setShowDiceMenu(false);
          setDiceEditing(false);
        }}
        title={t("chats.widgets.dice.defaultTitle")}
        rightAction={
          <button
            type="button"
            onClick={() => setDiceEditing((value) => !value)}
            aria-label={t("chats.widgets.dice.editNotation")}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-full border transition-colors",
              diceEditing
                ? "border-accent/40 bg-accent/15 text-accent"
                : "border-fg/10 bg-fg/5 text-fg/70 hover:border-fg/20 hover:bg-fg/10 hover:text-fg",
            )}
          >
            <Pencil size={14} />
          </button>
        }
      >
        <div className="space-y-3">
          {diceEditing && (
            <div className="flex items-center gap-2">
              <input
                value={diceNotation}
                onChange={(event) => setDiceNotation(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") setDiceEditing(false);
                }}
                placeholder="1d20"
                autoFocus
                spellCheck={false}
                autoCapitalize="none"
                className="flex-1 rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5 font-mono text-sm text-fg placeholder-fg/40 transition focus:border-accent/40 focus:outline-none"
              />
              <button
                type="button"
                onClick={() => setDiceEditing(false)}
                className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-accent/30 bg-accent/15 text-accent transition hover:bg-accent/25"
              >
                <Check size={16} />
              </button>
            </div>
          )}
          <WidgetContextProvider
            value={{
              ...widgetCtxValue,
              onInsertText: (text) => {
                widgetCtxValue.onInsertText(text);
                setShowDiceMenu(false);
                setDiceEditing(false);
              },
            }}
          >
            <WidgetDice node={{ id: "mobile-dice", type: "dice", notation: diceNotation }} />
          </WidgetContextProvider>
        </div>
      </BottomMenu>

      <AuthorNoteBottomMenu
        isOpen={showAuthorNoteMenu}
        onClose={() => setShowAuthorNoteMenu(false)}
        session={sessionForHeader ?? chatController.session}
        onSaved={setSessionForHeader}
      />

      <DynamicMemoryApprovalGate
        sessionId={chatController.session?.id ?? null}
        variant="chat"
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
        title={t("chats.chatBackground")}
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
                    alt={t("chats.background.currentAlt")}
                    className="h-32 w-full object-cover"
                  />
                  <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/70">
                    {hasSessionBackgroundOverride
                      ? t("chats.background.currentSession")
                      : hasSceneBackgroundDefault
                        ? t("chats.background.currentScene")
                        : t("chats.background.currentCharacterDefault")}
                  </div>
                </div>
              )}

              {hasSessionBackgroundOverride &&
                sceneBackgroundPreview &&
                sceneBackgroundPreview !== sessionBackgroundPreview && (
                  <div className="overflow-hidden rounded-2xl border border-fg/10 bg-fg/4">
                    <img
                      src={sceneBackgroundPreview}
                      alt={t("chats.background.sceneAlt")}
                      className="h-24 w-full object-cover"
                    />
                    <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/55">
                      {t("chats.background.sceneAlt")}
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
                      alt={t("chats.background.characterDefaultAlt")}
                      className="h-24 w-full object-cover"
                    />
                    <div className="border-t border-fg/10 px-4 py-3 text-sm text-fg/55">
                      {t("chats.background.characterDefaultAlt")}
                    </div>
                  </div>
                )}
            </div>
          )}

          <div className="space-y-2">
            <MenuButton
              icon={Image}
              title={hasSessionBackgroundOverride ? t("chats.background.replaceSession") : t("chats.background.uploadSession")}
              description={t("chats.background.onlyThisSession")}
              loading={savingSessionBackground}
              onClick={() => sessionBackgroundInputRef.current?.click()}
            />
            <MenuButton
              icon={Image}
              title={t("chats.background.chooseFromLibrary")}
              description={t("chats.background.chooseFromLibraryDesc")}
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
                    ? t("chats.background.useSceneDefault")
                    : hasCharacterBackgroundDefault
                      ? t("chats.background.useCharacterDefault")
                      : t("chats.background.removeBackground")
                }
                description={
                  hasSceneBackgroundDefault
                    ? t("chats.background.clearToSceneDesc")
                    : hasCharacterBackgroundDefault
                      ? t("chats.background.clearToCharacterDesc")
                    : t("chats.background.removeOverrideDesc")
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
        title={t("chats.background.chooseBackground")}
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
            {t("chats.helpMeReplyDraftPrompt")}
          </p>
          <MenuButton
            icon={PenLine}
            title={t("chats.useMyTextAsBase")}
            description={t("chats.useMyTextAsBaseDesc")}
            onClick={() => handleHelpMeReply("enrich")}
          />
          <MenuButton
            icon={Sparkles}
            title={t("chats.writeNewReply")}
            description={t("chats.writeNewReplyDesc")}
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
                  ? t("chats.helpMeReplyReasoning")
                  : t("chats.helpMeReplyWriting")}
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
              <span>{t("chats.sceneImage.regeneratePrompt")}</span>
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
              <span>{t("chats.useThisReply")}</span>
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
                  {t("chats.sceneImage.charCount", {
                    count: generatedScenePrompt.length.toLocaleString(),
                  })}
                </div>
              </div>
              <p className="line-clamp-6 whitespace-pre-wrap text-sm leading-relaxed text-white/82">
                {generatedScenePrompt}
              </p>
              <p className="mt-3 text-xs text-white/45">
                {t("chats.sceneImage.editPromptHint", {
                  action: t("chats.sceneImage.editPrompt").toLowerCase(),
                })}
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
              className={cn(
                "relative z-10 flex max-h-[92vh] w-full flex-col items-center gap-4",
                selectedImagePrompt
                  ? "max-w-[min(94vw,1380px)] lg:grid lg:grid-cols-[minmax(280px,340px)_minmax(0,1fr)] lg:items-stretch lg:gap-6"
                  : "max-w-[min(94vw,1100px)] lg:gap-6",
              )}
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
                      {t("chats.imagePrompt")}
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
                      {t("chats.imagePrompt")}
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

      {/* Desktop Appearance Drawer */}
      {!isMobile && (layoutCharacter ?? character) && (
        <ChatAppearanceDrawer
          open={appearanceDrawerOpen}
          onClose={() => setAppearanceDrawerOpen(false)}
          character={(layoutCharacter ?? character)!}
          onCharacterUpdate={() => reloadCharacter()}
          setDraftOverride={setDraftAppearanceOverride}
          registerFieldUpdater={registerAppearanceFieldUpdater}
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
  const { t } = useI18n();
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
          {character.description || character.definition || t("common.labels.noDescriptionYet")}
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
