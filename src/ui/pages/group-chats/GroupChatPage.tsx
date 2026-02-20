import { useEffect, useState, useRef, useCallback, useMemo, useLayoutEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { ChevronDown, Loader2, Sparkles, Image, RefreshCw, PenLine, Check } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

import { storageBridge } from "../../../core/storage/files";
import { generateGroupChatUserReply } from "../../../core/storage/repo";
import type {
  Character,
  GroupMessage,
  GroupParticipation,
  ImageAttachment,
  Model,
} from "../../../core/storage/schemas";
import { radius, interactive, cn } from "../../design-tokens";
import { useGroupChatLayoutContext } from "./GroupChatLayout";
import { splitThinkTags } from "../../../core/utils/thinkTags";

import { Routes } from "../../navigation";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import {
  GroupChatFooter,
  GroupChatHeader,
  GroupChatMessage,
  GroupChatMessageActionsBottomSheet,
  type VariantState,
} from "./components";

const MESSAGES_PAGE_SIZE = 50;
const STICKY_BOTTOM_THRESHOLD_PX = 80;

interface MessageActionState {
  message: GroupMessage;
  mode: "view" | "edit";
}

const isAbortMessage = (message: string) => {
  const normalized = message.toLowerCase();
  return (
    normalized.includes("aborted") ||
    normalized.includes("cancelled") ||
    normalized.includes("canceled")
  );
};

export function GroupChatPage() {
  const { groupSessionId } = useParams<{ groupSessionId: string }>();
  const navigate = useNavigate();

  // State variables
  const [messages, setMessages] = useState<GroupMessage[]>([]);
  const [_participationStats, setParticipationStats] = useState<GroupParticipation[]>([]);
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [sendingStatus, setSendingStatus] = useState<"selecting" | "generating" | null>(null);
  const [_selectedCharacterId, setSelectedCharacterId] = useState<string | null>(null);
  const assistantPlaceholderIdRef = useRef<string | null>(null);
  const [_selectedCharacterName, setSelectedCharacterName] = useState<string | null>(null);
  const [_selectedCharacterAvatarUrl, setSelectedCharacterAvatarUrl] = useState<string | null>(
    null,
  );
  const [regeneratingMessageId, setRegeneratingMessageId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  // Plus menu & Help Me Reply states
  const [showPlusMenu, setShowPlusMenu] = useState(false);
  const [showChoiceMenu, setShowChoiceMenu] = useState(false);
  const [showResultMenu, setShowResultMenu] = useState(false);
  const [generatedReply, setGeneratedReply] = useState<string | null>(null);
  const [generatingReply, setGeneratingReply] = useState(false);
  const [helpMeReplyError, setHelpMeReplyError] = useState<string | null>(null);
  const [shouldTriggerFileInput, setShouldTriggerFileInput] = useState(false);
  const [pendingAttachments, setPendingAttachments] = useState<ImageAttachment[]>([]);
  const [supportsImageInput, setSupportsImageInput] = useState(false);
  const helpMeReplyRequestIdRef = useRef<string | null>(null);
  const helpMeReplyUnlistenRef = useRef<UnlistenFn | null>(null);
  const helpMeReplyLoadingTimeoutRef = useRef<number | null>(null);

  // Message actions state
  const [messageAction, setMessageAction] = useState<MessageActionState | null>(null);
  const [editDraft, setEditDraft] = useState("");
  const [actionBusy, setActionBusy] = useState(false);
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [heldMessageId, setHeldMessageId] = useState<string | null>(null);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const isAtBottomRef = useRef(true);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const activeRequestIdRef = useRef<string | null>(null);
  const isGenerating = sending || regeneratingMessageId !== null;

  // Shared data from layout (stays mounted across sub-route navigations)
  const { session, sessionLoading, characters, personas, settings, backgroundImageData, theme, chatAppearance } =
    useGroupChatLayoutContext();
  const helpMeReplyEnabled = settings?.advancedSettings?.helpMeReplyEnabled ?? true;

  // Get current persona
  const currentPersona = useMemo(() => {
    if (!session?.personaId) return null;
    return personas.find((p) => p.id === session.personaId) || null;
  }, [session, personas]);

  // Load messages and stats (session, characters, personas, settings come from layout)
  const loadData = useCallback(async () => {
    if (!groupSessionId) return;

    try {
      const [msgs, stats] = await Promise.all([
        storageBridge.groupMessagesList(groupSessionId, MESSAGES_PAGE_SIZE),
        storageBridge.groupParticipationStats(groupSessionId),
      ]);

      setMessages(msgs);
      setParticipationStats(stats);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load messages");
    } finally {
      setLoading(false);
    }
  }, [groupSessionId]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  useEffect(() => {
    if (!error) return;

    const timer = setTimeout(() => {
      setError(null);
    }, 10000);

    return () => clearTimeout(timer);
  }, [error]);

  const lastMessageContentLength = messages[messages.length - 1]?.content.length ?? 0;

  const updateIsAtBottom = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return null;

    const { scrollTop, clientHeight, scrollHeight } = container;
    const atBottom = scrollTop + clientHeight >= scrollHeight - STICKY_BOTTOM_THRESHOLD_PX;
    isAtBottomRef.current = atBottom;
    setIsAtBottom((prev) => (prev === atBottom ? prev : atBottom));
    return scrollTop;
  }, []);

  useEffect(() => {
    if (!groupSessionId) return;

    const unlisten = listen<{
      sessionId: string;
      status: string;
      characterId?: string;
      characterName?: string;
    }>("group_chat_status", (event) => {
      const { sessionId, status, characterId, characterName } = event.payload;

      if (sessionId !== groupSessionId) return;

      if (status === "selecting_character") {
        setSendingStatus("selecting");
        setSelectedCharacterId(null);
        setSelectedCharacterName(null);
      } else if (status === "character_selected") {
        setSendingStatus("generating");
        setSelectedCharacterId(characterId || null);
        setSelectedCharacterName(characterName || null);
        // Look up character avatar
        const char = characters.find((c) => c.id === characterId);
        setSelectedCharacterAvatarUrl(char?.avatarPath || null);

        // Update the placeholder message with the selected character
        const placeholderId = assistantPlaceholderIdRef.current;
        if (placeholderId && characterId) {
          setMessages((prev) => {
            return prev.map((m) =>
              m.id === placeholderId ? { ...m, speakerCharacterId: characterId } : m,
            );
          });
        }
      } else if (status === "complete") {
        setSendingStatus(null);
        setSelectedCharacterId(null);
        setSelectedCharacterName(null);
        setSelectedCharacterAvatarUrl(null);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [groupSessionId, characters]);

  const handleScroll = useCallback(() => {
    updateIsAtBottom();
  }, [updateIsAtBottom]);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = "smooth") => {
    if (scrollContainerRef.current) {
      isAtBottomRef.current = true;
      setIsAtBottom(true);
      scrollContainerRef.current.scrollTo({
        top: scrollContainerRef.current.scrollHeight,
        behavior,
      });
    }
  }, []);

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
  }, [lastMessageContentLength, messages.length, isGenerating, updateIsAtBottom]);

  useEffect(() => {
    if (!isAtBottom || !isGenerating) return;
    scrollToBottom("auto");
  }, [isAtBottom, isGenerating, scrollToBottom]);

  const handleSend = useCallback(async () => {
    if (!groupSessionId || !draft.trim() || sending) return;

    const userMessage = draft.trim();
    const requestId = crypto.randomUUID();
    activeRequestIdRef.current = requestId;
    setDraft("");
    setSending(true);
    setSendingStatus("selecting");
    setError(null);

    // Create stable IDs for placeholders
    const userPlaceholderId = `temp-user-${Date.now()}`;
    const assistantPlaceholderId = `temp-assistant-${Date.now()}`;

    // Store the assistant placeholder ID in ref so the status listener can find it
    assistantPlaceholderIdRef.current = assistantPlaceholderId;

    // Optimistic update - add user message
    const tempUserMessage: GroupMessage = {
      id: userPlaceholderId,
      sessionId: groupSessionId,
      role: "user",
      content: userMessage,
      speakerCharacterId: null,
      turnNumber: messages.length + 1,
      createdAt: Date.now(),
      usage: undefined,
      variants: undefined,
      selectedVariantId: undefined,
      isPinned: false,
      attachments: [],
      reasoning: null,
      selectionReasoning: null,
    };

    // Create placeholder assistant message (will be updated with character when selected)
    const tempAssistantMessage: GroupMessage = {
      id: assistantPlaceholderId,
      sessionId: groupSessionId,
      role: "assistant",
      content: "",
      speakerCharacterId: null, // Will be set when character_selected event fires
      turnNumber: messages.length + 2,
      createdAt: Date.now(),
      usage: undefined,
      variants: undefined,
      selectedVariantId: undefined,
      isPinned: false,
      attachments: [],
      reasoning: null,
      selectionReasoning: null,
    };

    // Add both user and assistant placeholder immediately
    setMessages((prev) => [...prev, tempUserMessage, tempAssistantMessage]);
    scrollToBottom();

    let unlistenNormalized: (() => void) | null = null;

    try {
      // Listen for streaming updates
      unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
        try {
          const payload =
            typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;

          if (payload && payload.type === "delta" && payload.data?.text) {
            // Update the placeholder message with streamed content
            setMessages((prev) => {
              return prev.map((m) =>
                m.id === assistantPlaceholderId
                  ? { ...m, content: m.content + String(payload.data.text) }
                  : m,
              );
            });
          } else if (payload && payload.type === "reasoning" && payload.data?.text) {
            // Update reasoning
            setMessages((prev) => {
              return prev.map((m) =>
                m.id === assistantPlaceholderId
                  ? { ...m, reasoning: (m.reasoning || "") + String(payload.data.text) }
                  : m,
              );
            });
          } else if (payload && payload.type === "error" && payload.data?.message) {
            const message = String(payload.data.message);
            if (!isAbortMessage(message)) {
              setError(message);
            }
          }
        } catch {
          // ignore malformed payloads
        }
      });

      const response = await storageBridge.groupChatSend(
        groupSessionId,
        userMessage,
        true,
        requestId,
      );

      // Update messages with actual saved messages
      const updatedMessages = await storageBridge.groupMessagesList(
        groupSessionId,
        MESSAGES_PAGE_SIZE,
      );
      console.log("🔍 After send - updated messages count:", updatedMessages.length);
      console.log("🔍 Last message after send:", updatedMessages[updatedMessages.length - 1]);
      console.log("🔍 Last message modelId:", updatedMessages[updatedMessages.length - 1]?.modelId);
      setMessages(updatedMessages);
      setParticipationStats(response.participationStats);
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err);
      if (isAbortMessage(errMsg)) {
        setError(null);
        try {
          const updatedMessages = await storageBridge.groupMessagesList(
            groupSessionId,
            MESSAGES_PAGE_SIZE,
          );
          setMessages(updatedMessages);
        } catch (loadErr) {
          console.error("Failed to refresh messages after abort:", loadErr);
          setMessages((prev) => prev.filter((m) => m.id !== assistantPlaceholderId));
        }
        return;
      }
      console.error("Failed to send message:", err);
      setError(errMsg || "Failed to send message");
      // Remove optimistic messages on error
      setMessages((prev) =>
        prev.filter((m) => m.id !== userPlaceholderId && m.id !== assistantPlaceholderId),
      );
    } finally {
      if (unlistenNormalized) unlistenNormalized();
      assistantPlaceholderIdRef.current = null;
      if (activeRequestIdRef.current === requestId) {
        activeRequestIdRef.current = null;
      }
      setSending(false);
      setSendingStatus(null);
      setSelectedCharacterId(null);
      setSelectedCharacterName(null);
      setSelectedCharacterAvatarUrl(null);
    }
  }, [groupSessionId, draft, sending, messages.length, scrollToBottom]);

  const handleRegenerate = useCallback(
    async (messageId: string, forceCharacterId?: string) => {
      if (!groupSessionId || regeneratingMessageId) return;

      const requestId = crypto.randomUUID();
      activeRequestIdRef.current = requestId;
      setRegeneratingMessageId(messageId);
      setError(null);

      // Clear the message content to show streaming
      setMessages((prev) =>
        prev.map((m) => (m.id === messageId ? { ...m, content: "", reasoning: null } : m)),
      );

      let unlistenNormalized: (() => void) | null = null;

      try {
        // Listen for streaming updates
        unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
          try {
            const payload =
              typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;

            if (payload && payload.type === "delta" && payload.data?.text) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === messageId ? { ...m, content: m.content + String(payload.data.text) } : m,
                ),
              );
            } else if (payload && payload.type === "reasoning" && payload.data?.text) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === messageId
                    ? { ...m, reasoning: (m.reasoning || "") + String(payload.data.text) }
                    : m,
                ),
              );
            } else if (payload && payload.type === "error" && payload.data?.message) {
              const message = String(payload.data.message);
              if (!isAbortMessage(message)) {
                setError(message);
              }
            }
          } catch {
            // ignore malformed payloads
          }
        });

        const response = await storageBridge.groupChatRegenerate(
          groupSessionId,
          messageId,
          forceCharacterId,
          requestId,
        );

        // Update messages with final saved data
        const updatedMessages = await storageBridge.groupMessagesList(
          groupSessionId,
          MESSAGES_PAGE_SIZE,
        );
        setMessages(updatedMessages);
        setParticipationStats(response.participationStats);
      } catch (err) {
        console.error("Failed to regenerate:", err);
        const errMsg = err instanceof Error ? err.message : String(err);
        if (!isAbortMessage(errMsg)) {
          setError(errMsg || "Failed to regenerate");
        } else {
          setError(null);
        }
        // Reload messages to restore original content on error
        const updatedMessages = await storageBridge.groupMessagesList(
          groupSessionId,
          MESSAGES_PAGE_SIZE,
        );
        setMessages(updatedMessages);
      } finally {
        if (unlistenNormalized) unlistenNormalized();
        setRegeneratingMessageId(null);
        if (activeRequestIdRef.current === requestId) {
          activeRequestIdRef.current = null;
        }
      }
    },
    [groupSessionId, regeneratingMessageId],
  );

  const handleContinue = useCallback(
    async (forceCharacterId?: string) => {
      if (!groupSessionId || sending) return;

      const requestId = crypto.randomUUID();
      activeRequestIdRef.current = requestId;
      const assistantPlaceholderId = `temp-continue-${Date.now()}`;

      setSending(true);
      setSendingStatus("selecting");
      setError(null);

      // Store placeholder ID for status listener
      assistantPlaceholderIdRef.current = assistantPlaceholderId;

      // Create placeholder assistant message
      const tempAssistantMessage: GroupMessage = {
        id: assistantPlaceholderId,
        sessionId: groupSessionId,
        role: "assistant",
        content: "",
        speakerCharacterId: null,
        turnNumber: messages.length + 1,
        createdAt: Date.now(),
        usage: undefined,
        variants: undefined,
        selectedVariantId: undefined,
        isPinned: false,
        attachments: [],
        reasoning: null,
        selectionReasoning: null,
      };

      setMessages((prev) => [...prev, tempAssistantMessage]);
      scrollToBottom();

      let unlistenNormalized: (() => void) | null = null;

      try {
        // Listen for streaming updates
        unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
          try {
            const payload =
              typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;

            if (payload && payload.type === "delta" && payload.data?.text) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantPlaceholderId
                    ? { ...m, content: m.content + String(payload.data.text) }
                    : m,
                ),
              );
            } else if (payload && payload.type === "reasoning" && payload.data?.text) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantPlaceholderId
                    ? { ...m, reasoning: (m.reasoning || "") + String(payload.data.text) }
                    : m,
                ),
              );
            } else if (payload && payload.type === "error" && payload.data?.message) {
              const message = String(payload.data.message);
              if (!isAbortMessage(message)) {
                setError(message);
              }
            }
          } catch {
            // ignore malformed payloads
          }
        });

        const response = await storageBridge.groupChatContinue(
          groupSessionId,
          forceCharacterId,
          requestId,
        );

        // Update messages with final saved data
        const updatedMessages = await storageBridge.groupMessagesList(
          groupSessionId,
          MESSAGES_PAGE_SIZE,
        );
        setMessages(updatedMessages);
        setParticipationStats(response.participationStats);
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        if (isAbortMessage(errMsg)) {
          setError(null);
          try {
            const updatedMessages = await storageBridge.groupMessagesList(
              groupSessionId,
              MESSAGES_PAGE_SIZE,
            );
            setMessages(updatedMessages);
          } catch (loadErr) {
            console.error("Failed to refresh messages after abort:", loadErr);
            setMessages((prev) => prev.filter((m) => m.id !== assistantPlaceholderId));
          }
          return;
        }
        console.error("Failed to continue:", err);
        setError(errMsg || "Failed to continue");
        // Remove placeholder on error
        setMessages((prev) => prev.filter((m) => m.id !== assistantPlaceholderId));
      } finally {
        if (unlistenNormalized) unlistenNormalized();
        assistantPlaceholderIdRef.current = null;
        if (activeRequestIdRef.current === requestId) {
          activeRequestIdRef.current = null;
        }
        setSending(false);
        setSendingStatus(null);
        setSelectedCharacterId(null);
        setSelectedCharacterName(null);
        setSelectedCharacterAvatarUrl(null);
      }
    },
    [groupSessionId, sending, messages.length, scrollToBottom],
  );

  const handleAbort = useCallback(async () => {
    const requestId = activeRequestIdRef.current;
    if (!requestId) return;

    try {
      await storageBridge.abortRequest(requestId);
    } catch (err) {
      console.error("Failed to abort group chat request:", err);
    }
  }, []);

  const getCharacterById = useCallback(
    (characterId?: string | null): Character | undefined => {
      if (!characterId) return undefined;
      return characters.find((c) => c.id === characterId);
    },
    [characters],
  );

  // Variant state management for drag-to-change-variants
  const getVariantState = useCallback((message: GroupMessage): VariantState => {
    const variants = message.variants ?? [];
    if (variants.length === 0) {
      return {
        variants: [],
        selectedIndex: -1,
        total: 0,
      };
    }
    const explicitIndex = message.selectedVariantId
      ? variants.findIndex((variant) => variant.id === message.selectedVariantId)
      : -1;
    const selectedIndex = explicitIndex >= 0 ? explicitIndex : variants.length - 1;
    return {
      variants,
      selectedIndex,
      total: variants.length,
    };
  }, []);

  const handleVariantSwipe = useCallback(
    async (messageId: string, direction: "prev" | "next") => {
      const message = messages.find((m) => m.id === messageId);
      if (!message) return;

      const variantState = getVariantState(message);
      if (variantState.total <= 1) return;

      const currentIndex = variantState.selectedIndex;
      let nextIndex: number;

      if (direction === "prev") {
        nextIndex = currentIndex > 0 ? currentIndex - 1 : variantState.total - 1;
      } else {
        nextIndex = currentIndex < variantState.total - 1 ? currentIndex + 1 : 0;
      }

      const variants = variantState.variants ?? [];
      const nextVariant = variants[nextIndex];
      if (!nextVariant) return;

      try {
        await storageBridge.groupMessageSelectVariant(messageId, nextVariant.id);
        // Refresh messages to get updated content
        const updatedMessages = await storageBridge.groupMessagesList(
          groupSessionId!,
          MESSAGES_PAGE_SIZE,
        );
        setMessages(updatedMessages);
      } catch (err) {
        console.error("Failed to select variant:", err);
      }
    },
    [messages, getVariantState, groupSessionId],
  );

  const handleVariantDrag = useCallback(
    (messageId: string, offsetX: number) => {
      if (offsetX > 60) {
        void handleVariantSwipe(messageId, "prev");
      } else if (offsetX < -60) {
        void handleVariantSwipe(messageId, "next");
      }
    },
    [handleVariantSwipe],
  );

  // Message action handlers
  const openMessageActions = useCallback((message: GroupMessage) => {
    setMessageAction({ message, mode: "view" });
    setHeldMessageId(message.id);
    setActionError(null);
    setActionStatus(null);
  }, []);

  const closeMessageActions = useCallback(() => {
    setMessageAction(null);
    setHeldMessageId(null);
    setEditDraft("");
    setActionError(null);
    setActionStatus(null);
  }, []);

  const handleCopyMessage = useCallback(async () => {
    if (!messageAction) return;
    try {
      await navigator.clipboard?.writeText(messageAction.message.content);
      setActionStatus("Copied!");
      setTimeout(() => setActionStatus(null), 1500);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Failed to copy");
    }
  }, [messageAction]);

  const handleDeleteMessage = useCallback(async () => {
    if (!messageAction || !groupSessionId) return;

    setActionBusy(true);
    try {
      await storageBridge.groupMessageDelete(groupSessionId, messageAction.message.id);
      const updatedMessages = await storageBridge.groupMessagesList(
        groupSessionId,
        MESSAGES_PAGE_SIZE,
      );
      setMessages(updatedMessages);
      closeMessageActions();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Failed to delete");
    } finally {
      setActionBusy(false);
    }
  }, [messageAction, groupSessionId, closeMessageActions]);

  const handleRewindToMessage = useCallback(async () => {
    if (!messageAction || !groupSessionId) return;

    setActionBusy(true);
    try {
      await storageBridge.groupMessagesDeleteAfter(groupSessionId, messageAction.message.id);
      const updatedMessages = await storageBridge.groupMessagesList(
        groupSessionId,
        MESSAGES_PAGE_SIZE,
      );
      setMessages(updatedMessages);
      const stats = await storageBridge.groupParticipationStats(groupSessionId);
      setParticipationStats(stats);
      closeMessageActions();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Failed to rewind");
    } finally {
      setActionBusy(false);
    }
  }, [messageAction, groupSessionId, closeMessageActions]);

  const handleSaveEdit = useCallback(async () => {
    if (!messageAction || !groupSessionId || !editDraft.trim()) return;

    setActionBusy(true);
    try {
      const updatedMessage = {
        ...messageAction.message,
        content: editDraft.trim(),
      };
      await storageBridge.groupMessageUpsert(groupSessionId, updatedMessage);
      const updatedMessages = await storageBridge.groupMessagesList(
        groupSessionId,
        MESSAGES_PAGE_SIZE,
      );
      setMessages(updatedMessages);
      closeMessageActions();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Failed to save");
    } finally {
      setActionBusy(false);
    }
  }, [messageAction, groupSessionId, editDraft, closeMessageActions]);

  // Check if all group characters support image input
  useEffect(() => {
    const checkImageSupport = async () => {
      if (!session || !settings || !characters.length) {
        setSupportsImageInput(false);
        return;
      }

      try {
        const groupChars = session.characterIds
          .map((id) => characters.find((c) => c.id === id))
          .filter(Boolean) as Character[];

        if (groupChars.length === 0) {
          setSupportsImageInput(false);
          return;
        }

        // Check if ALL characters support image input
        const allSupport = groupChars.every((char) => {
          const effectiveModelId = char.defaultModelId || settings.defaultModelId;
          const model = settings.models.find((m: Model) => m.id === effectiveModelId);
          return model?.inputScopes?.includes("image") ?? false;
        });

        setSupportsImageInput(allSupport);
      } catch (err) {
        console.error("Failed to check image support:", err);
        setSupportsImageInput(false);
      }
    };

    checkImageSupport();
  }, [session, settings, characters]);

  // Plus menu handlers
  const handleOpenPlusMenu = useCallback(() => {
    setShowPlusMenu(true);
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
    if (!requestId) return;
    try {
      await storageBridge.abortRequest(requestId);
    } catch (err) {
      console.error("Failed to abort Help Me Reply request:", err);
    }
  }, [clearHelpMeReplyRuntime]);

  const handleCloseHelpMeReplyResultMenu = useCallback(() => {
    setShowResultMenu(false);
    setGeneratedReply(null);
    setHelpMeReplyError(null);
    void cancelHelpMeReplyGeneration();
  }, [cancelHelpMeReplyGeneration]);

  const handleHelpMeReply = useCallback(
    async (mode: "new" | "enrich") => {
      if (!session?.id) return;

      // Close other menus and show result menu with loading state immediately
      setShowChoiceMenu(false);
      setShowPlusMenu(false);
      setGeneratedReply(null);
      setHelpMeReplyError(null);
      setGeneratingReply(true);
      setShowResultMenu(true);

      const requestId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      helpMeReplyRequestIdRef.current = requestId;
      let streamingText = "";
      let hasStartedStreaming = false;

      // Timeout to clear loading state if streaming doesn't start within 5 seconds
      helpMeReplyLoadingTimeoutRef.current = window.setTimeout(() => {
        if (!hasStartedStreaming) {
          setGeneratingReply(false);
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
                if (helpMeReplyLoadingTimeoutRef.current !== null) {
                  window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
                  helpMeReplyLoadingTimeoutRef.current = null;
                }
              }
              streamingText += String(payload.data.text);
              setGeneratedReply(streamingText);
            } else if (payload && payload.type === "error") {
              const message =
                payload.data?.message ||
                payload.data?.error ||
                payload.message ||
                "Help Me Reply failed.";
              setHelpMeReplyError(String(message));
              setGeneratingReply(false);
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
        const result = await generateGroupChatUserReply(session.id, currentDraft, requestId);

        // If we didn't get streaming updates, use the final result
        if (!streamingText.trim()) {
          if (result?.trim()) {
            setGeneratedReply(result);
          } else {
            setHelpMeReplyError("Help Me Reply failed to generate a reply.");
          }
        }

        // Clear loading state once API call completes (for non-streaming case)
        if (!hasStartedStreaming) {
          setGeneratingReply(false);
          if (helpMeReplyLoadingTimeoutRef.current !== null) {
            window.clearTimeout(helpMeReplyLoadingTimeoutRef.current);
            helpMeReplyLoadingTimeoutRef.current = null;
          }
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setHelpMeReplyError(message);
      } finally {
        // Only clear loading if streaming hasn't started yet
        if (!hasStartedStreaming) {
          setGeneratingReply(false);
        }
        if (helpMeReplyRequestIdRef.current === requestId) {
          clearHelpMeReplyRuntime();
        }
      }
    },
    [session?.id, draft, clearHelpMeReplyRuntime],
  );

  const handleUseReply = useCallback(() => {
    if (generatedReply) {
      setDraft(generatedReply);
    }
    setShowResultMenu(false);
    setGeneratedReply(null);
    setHelpMeReplyError(null);
  }, [generatedReply]);

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

  useEffect(
    () => () => {
      void cancelHelpMeReplyGeneration();
    },
    [cancelHelpMeReplyGeneration],
  );

  const addPendingAttachment = useCallback((attachment: ImageAttachment) => {
    setPendingAttachments((prev) => [...prev, attachment]);
  }, []);

  const removePendingAttachment = useCallback((attachmentId: string) => {
    setPendingAttachments((prev) => prev.filter((a) => a.id !== attachmentId));
  }, []);

  const groupCharacters = useMemo(() => {
    if (!session) return [];
    return session.characterIds
      .map((id) => characters.find((c) => c.id === id))
      .filter(Boolean) as Character[];
  }, [session, characters]);

  if (sessionLoading || loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-fg/50" />
      </div>
    );
  }

  if (!session) {
    return (
      <div className="flex h-full flex-col items-center justify-center p-4">
        <p className="text-fg/50 mb-4">{error || "Group session not found"}</p>
        <button
          onClick={() => navigate(Routes.groupChats)}
          className={cn(
            "px-4 py-2",
            radius.md,
            "border border-fg/15 bg-fg/10 text-fg",
            interactive.transition.fast,
          )}
        >
          Back to Group Chats
        </button>
      </div>
    );
  }

  return (
    <div
      className="relative flex h-screen flex-col overflow-hidden"
      style={{ backgroundColor: backgroundImageData ? undefined : "#050505" }}
    >
      {/* Background image */}
      {backgroundImageData && (
        <div
          className="pointer-events-none fixed inset-0 z-0"
          style={{
            backgroundImage: `url(${backgroundImageData})`,
            backgroundSize: "cover",
            backgroundPosition: "center",
            backgroundRepeat: "no-repeat",
          }}
        />
      )}

      {/* Content layer - on top of background */}
      <div
        className="relative z-10 flex h-full flex-col"
        style={{ paddingTop: "env(safe-area-inset-top)" }}
      >
        {/* Header */}
        <div className="relative z-20 shrink-0">
          <GroupChatHeader
            session={session}
            characters={groupCharacters}
            onBack={() => navigate(Routes.groupChats)}
            onSettings={() => navigate(Routes.groupChatSettings(session.id))}
            onMemories={() => navigate(Routes.groupChatMemories(session.id))}
            hasBackgroundImage={!!backgroundImageData}
          />
        </div>

        {/* Main content area - flex-1 takes remaining space */}
        <main
          ref={scrollContainerRef}
          onScroll={handleScroll}
          className="relative flex-1 overflow-y-auto px-2 pb-2"
        >
          <div className={`${chatAppearance.messageGap === "tight" ? "space-y-2" : chatAppearance.messageGap === "relaxed" ? "space-y-6" : "space-y-4"} pb-6 pt-4`}>
            {messages.length === 0 ? (
              <div className="flex min-h-[50vh] items-center justify-center">
                <p className="text-fg/30 text-center">
                  Start a conversation with {groupCharacters.map((c) => c.name).join(", ")}
                </p>
              </div>
            ) : (
              <AnimatePresence initial={false}>
                {messages.map((message, index) => {
                  const parsed = splitThinkTags(message.content);
                  const combinedReasoning = [message.reasoning ?? "", parsed.reasoning]
                    .filter(Boolean)
                    .join("\n");
                  return (
                    <GroupChatMessage
                      key={message.id}
                      message={message}
                      index={index}
                      messagesLength={messages.length}
                      heldMessageId={heldMessageId}
                      regeneratingMessageId={regeneratingMessageId}
                      sending={sending}
                      character={getCharacterById(message.speakerCharacterId)}
                      persona={currentPersona}
                      characters={groupCharacters}
                      theme={theme}
                      chatAppearance={chatAppearance}
                      getVariantState={getVariantState}
                      handleVariantDrag={handleVariantDrag}
                      handleRegenerate={async (msg) => {
                        await handleRegenerate(msg.id);
                      }}
                      onLongPress={(msg) => openMessageActions(msg)}
                      displayContent={parsed.content}
                      reasoning={combinedReasoning || undefined}
                    />
                  );
                })}
              </AnimatePresence>
            )}

            {/* Sending Indicator - only show during selection phase */}
            {sending && sendingStatus === "selecting" && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="flex items-center gap-2 mt-4 text-fg/50"
              >
                <Loader2 className="h-4 w-4 animate-spin" />
                <span className="text-sm">Selecting character...</span>
              </motion.div>
            )}
          </div>
        </main>

        {/* Scroll to Bottom Button */}
        <AnimatePresence>
          {!isAtBottom && (
            <motion.button
              type="button"
              aria-label="Scroll to bottom"
              onClick={() => scrollToBottom("smooth")}
              initial={{ opacity: 0, y: 10, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 10, scale: 0.98 }}
              transition={{ duration: 0.18, ease: "easeOut" }}
              className={cn(
                "fixed right-3 z-30 flex h-11 w-11 items-center justify-center",
                "bottom-[calc(env(safe-area-inset-bottom)+88px)]",
                "border border-fg/15 bg-surface-el/40 text-fg/80 shadow-lg backdrop-blur-sm",
                "hover:bg-surface-el/55 active:scale-95",
                radius.full,
              )}
            >
              <ChevronDown size={18} />
            </motion.button>
          )}
        </AnimatePresence>

        {/* Footer */}
        <div
          className="relative z-20 shrink-0"
          style={{ paddingBottom: "env(safe-area-inset-bottom)" }}
        >
          <GroupChatFooter
            draft={draft}
            setDraft={setDraft}
            error={error}
            setError={setError}
            sending={sending}
            characters={groupCharacters}
            persona={currentPersona}
            onSendMessage={handleSend}
            onContinue={messages.length > 0 ? () => handleContinue() : undefined}
            onAbort={handleAbort}
            hasBackgroundImage={!!backgroundImageData}
            pendingAttachments={pendingAttachments}
            onAddAttachment={supportsImageInput ? addPendingAttachment : undefined}
            onRemoveAttachment={supportsImageInput ? removePendingAttachment : undefined}
            onOpenPlusMenu={handleOpenPlusMenu}
            triggerFileInput={shouldTriggerFileInput}
            onFileInputTriggered={() => setShouldTriggerFileInput(false)}
          />
        </div>
      </div>

      {/* Plus Menu - Upload Image & Help Me Reply */}
      <BottomMenu isOpen={showPlusMenu} onClose={() => setShowPlusMenu(false)} title="Add Content">
        <div className="space-y-2">
          {supportsImageInput && (
            <MenuButton icon={Image} title="Upload Image" onClick={handlePlusMenuImageUpload} />
          )}
          {helpMeReplyEnabled && (
            <MenuButton
              icon={Sparkles}
              title="Help Me Reply"
              description="Let AI suggest what to say"
              onClick={handlePlusMenuHelpMeReply}
            />
          )}
        </div>
      </BottomMenu>

      {/* Choice Menu - Use existing draft or generate new */}
      <BottomMenu
        isOpen={showChoiceMenu}
        onClose={() => setShowChoiceMenu(false)}
        title="Help Me Reply"
      >
        <div className="space-y-2">
          <p className="text-sm text-fg/60 mb-4">
            You have a draft message. How would you like to proceed?
          </p>
          <MenuButton
            icon={PenLine}
            title="Use my text as base"
            description="Expand and improve your draft"
            onClick={() => handleHelpMeReply("enrich")}
          />
          <MenuButton
            icon={Sparkles}
            title="Write something new"
            description="Generate a fresh reply"
            onClick={() => handleHelpMeReply("new")}
          />
        </div>
      </BottomMenu>

      {/* Result Menu - Show generated reply with Regenerate/Use options */}
      <BottomMenu
        isOpen={showResultMenu}
        onClose={handleCloseHelpMeReplyResultMenu}
        title="Suggested Reply"
      >
        <div className="space-y-4">
          {helpMeReplyError ? (
            <div className="bg-danger/10 border border-danger/30 rounded-lg p-4">
              <p className="text-danger text-sm">{helpMeReplyError}</p>
            </div>
          ) : generatingReply && !generatedReply ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-8 w-8 animate-spin text-fg/50" />
            </div>
          ) : generatedReply ? (
            <div
              className={cn(
                "bg-fg/5 border border-fg/10 p-4",
                radius.lg,
                "max-h-[40vh] overflow-y-auto",
              )}
            >
              <p className="text-fg/90 text-sm whitespace-pre-wrap">{generatedReply}</p>
            </div>
          ) : null}

          <div className="flex gap-3">
            <button
              onClick={() => handleHelpMeReply(draft.trim() ? "enrich" : "new")}
              disabled={generatingReply}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-3 px-4",
                radius.lg,
                "bg-fg/10 text-fg/80 hover:bg-fg/15",
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
                "bg-accent text-fg hover:bg-accent/80",
                "disabled:opacity-50 transition-all",
              )}
            >
              <Check size={18} />
              <span>Use Reply</span>
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* Message Actions Bottom Sheet */}
      <GroupChatMessageActionsBottomSheet
        messageAction={messageAction}
        actionError={actionError}
        actionStatus={actionStatus}
        actionBusy={actionBusy}
        editDraft={editDraft}
        setEditDraft={setEditDraft}
        closeMessageActions={closeMessageActions}
        setActionError={setActionError}
        setActionStatus={setActionStatus}
        handleSaveEdit={handleSaveEdit}
        handleDeleteMessage={handleDeleteMessage}
        handleRewindToMessage={handleRewindToMessage}
        handleCopyMessage={handleCopyMessage}
        setMessageAction={setMessageAction}
        onRegenerate={(charId) => {
          closeMessageActions();
          if (messageAction) {
            handleRegenerate(messageAction.message.id, charId);
          }
        }}
        characters={groupCharacters}
      />
    </div>
  );
}
