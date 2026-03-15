import { useCallback, useReducer, useRef, useEffect } from "react";

import { useChatAbortController } from "./useChatAbortController";
import { isStartingSceneMessage, queueSessionSave } from "./chatControllerShared";
import { chatReducer, initialChatState, type MessageActionState } from "./chatReducer";
import { useChatEnhancementsController } from "./useChatEnhancementsController";
import {
  useChatMessageActionsController,
  type VariantState,
} from "./useChatMessageActionsController";
import { useChatSessionController } from "./useChatSessionController";
import { useChatStreamingController } from "./useChatStreamingController";
import { logManager } from "../../../../core/utils/logger";
import type {
  Character,
  Persona,
  Session,
  StoredMessage,
  ImageAttachment,
} from "../../../../core/storage/schemas";

export interface ChatController {
  // State
  character: Character | null;
  persona: Persona | null;
  session: Session | null;
  messages: StoredMessage[];
  draft: string;
  loading: boolean;
  sending: boolean;
  error: string | null;
  messageAction: MessageActionState | null;
  actionError: string | null;
  actionStatus: string | null;
  actionBusy: boolean;
  editDraft: string;
  heldMessageId: string | null;
  regeneratingMessageId: string | null;
  activeRequestId: string | null;
  pendingAttachments: ImageAttachment[];
  streamingReasoning: Record<string, string>;
  hasMoreMessagesBefore: boolean;
  loadOlderMessages: () => Promise<void>;
  ensureMessageLoaded: (messageId: string) => Promise<void>;

  // Setters
  setDraft: (value: string) => void;
  setError: (value: string | null) => void;
  setMessageAction: (value: MessageActionState | null) => void;
  setActionError: (value: string | null) => void;
  setActionStatus: (value: string | null) => void;
  setActionBusy: (value: boolean) => void;
  setEditDraft: (value: string) => void;
  setHeldMessageId: (value: string | null) => void;
  setPendingAttachments: (attachments: ImageAttachment[]) => void;
  addPendingAttachment: (attachment: ImageAttachment) => void;
  removePendingAttachment: (attachmentId: string) => void;
  clearPendingAttachments: () => void;

  // Actions
  handleSend: (
    message: string,
    attachments?: ImageAttachment[],
    options?: { swapPlaces?: boolean },
  ) => Promise<void>;
  handleContinue: (options?: { swapPlaces?: boolean }) => Promise<void>;
  handleRegenerate: (message: StoredMessage, options?: { swapPlaces?: boolean }) => Promise<void>;
  handleAbort: () => Promise<void>;
  getVariantState: (message: StoredMessage) => VariantState;
  applyVariantSelection: (messageId: string, variantId: string) => Promise<void>;
  handleVariantSwipe: (messageId: string, direction: "prev" | "next") => Promise<void>;
  handleVariantDrag: (messageId: string, offsetX: number) => Promise<void>;
  handleSaveEdit: () => Promise<void>;
  handleDeleteMessage: (message: StoredMessage) => Promise<void>;
  handleRewindToMessage: (message: StoredMessage) => Promise<void>;
  handleBranchFromMessage: (message: StoredMessage) => Promise<string | null>;
  handleBranchToCharacter: (
    message: StoredMessage,
    targetCharacterId: string,
  ) => Promise<{ sessionId: string; characterId: string } | null>;
  handleTogglePin: (message: StoredMessage) => Promise<void>;
  resetMessageActions: () => void;
  initializeLongPressTimer: (id: number | null) => void;
  isStartingSceneMessage: (message: StoredMessage) => boolean;
}

export function useChatController(
  characterId?: string,
  options: { sessionId?: string } = {},
): ChatController {
  const log = logManager({ component: "useChatController" });
  const [state, dispatch] = useReducer(chatReducer, initialChatState);
  const { sessionId } = options;

  const messagesRef = useRef<StoredMessage[]>([]);
  const hasMoreMessagesBeforeRef = useRef<boolean>(true);
  const loadingOlderRef = useRef<boolean>(false);
  const sessionOperationRef = useRef<boolean>(false);
  const lastKnownSessionTimestampRef = useRef<number>(0);
  const recordSessionTimestamp = useCallback((updatedAt: number) => {
    lastKnownSessionTimestampRef.current = updatedAt;
  }, []);
  const persistSession = useCallback(
    async (session: Session) => {
      sessionOperationRef.current = true;
      try {
        await queueSessionSave(session);
        recordSessionTimestamp(session.updatedAt);
      } finally {
        sessionOperationRef.current = false;
      }
    },
    [recordSessionTimestamp],
  );
  const controllerContext = {
    state,
    dispatch,
    messagesRef,
    sessionOperationRef,
    log,
    persistSession,
    recordSessionTimestamp,
  };
  const pagingContext = {
    ...controllerContext,
    hasMoreMessagesBeforeRef,
    loadingOlderRef,
  };
  const { initializeLongPressTimer, runInChatImageGeneration, triggerTypingHaptic } =
    useChatEnhancementsController({
      context: controllerContext,
    });

  const { reloadSessionStateFromStorage, loadOlderMessages, ensureMessageLoaded } =
    useChatSessionController({
      context: pagingContext,
      characterId,
      sessionId,
    });

  const { handleSend, handleContinue, handleRegenerate } = useChatStreamingController({
    context: pagingContext,
    runInChatImageGeneration,
    reloadSessionStateFromStorage,
    triggerTypingHaptic,
  });
  const {
    resetMessageActions,
    getVariantState,
    applyVariantSelection,
    handleVariantSwipe,
    handleVariantDrag,
    handleSaveEdit,
    handleDeleteMessage,
    handleRewindToMessage,
    handleTogglePin,
    handleBranchFromMessage,
    handleBranchToCharacter,
  } = useChatMessageActionsController({
    context: controllerContext,
  });
  const { handleAbort } = useChatAbortController({
    context: controllerContext,
  });

  useEffect(() => {
    const sessionId = state.session?.id;
    if (!sessionId) return;

    const key = `chat_draft_${sessionId}`;
    if (state.draft.trim()) {
      localStorage.setItem(key, JSON.stringify({ text: state.draft, updatedAt: Date.now() }));
    } else {
      localStorage.removeItem(key);
    }
  }, [state.session?.id, state.draft]);

  return {
    // State
    character: state.character,
    persona: state.persona,
    session: state.session,
    messages: state.messages,
    draft: state.draft,
    loading: state.loading,
    sending: state.sending,
    error: state.error,
    messageAction: state.messageAction,
    actionError: state.actionError,
    actionStatus: state.actionStatus,
    actionBusy: state.actionBusy,
    editDraft: state.editDraft,
    heldMessageId: state.heldMessageId,
    regeneratingMessageId: state.regeneratingMessageId,
    activeRequestId: state.activeRequestId,
    pendingAttachments: state.pendingAttachments,
    streamingReasoning: state.streamingReasoning,
    hasMoreMessagesBefore: hasMoreMessagesBeforeRef.current,

    // Setters
    setDraft: useCallback((value: string) => dispatch({ type: "SET_DRAFT", payload: value }), [dispatch]),
    setError: useCallback(
      (value: string | null) => dispatch({ type: "SET_ERROR", payload: value }),
      [],
    ),
    setMessageAction: useCallback(
      (value: MessageActionState | null) =>
        dispatch({ type: "SET_MESSAGE_ACTION", payload: value }),
      [],
    ),
    setActionError: useCallback(
      (value: string | null) => dispatch({ type: "SET_ACTION_ERROR", payload: value }),
      [],
    ),
    setActionStatus: useCallback(
      (value: string | null) => dispatch({ type: "SET_ACTION_STATUS", payload: value }),
      [],
    ),
    setActionBusy: useCallback(
      (value: boolean) => dispatch({ type: "SET_ACTION_BUSY", payload: value }),
      [],
    ),
    setEditDraft: useCallback(
      (value: string) => dispatch({ type: "SET_EDIT_DRAFT", payload: value }),
      [],
    ),
    setHeldMessageId: useCallback(
      (value: string | null) => dispatch({ type: "SET_HELD_MESSAGE_ID", payload: value }),
      [],
    ),
    setPendingAttachments: useCallback(
      (attachments: ImageAttachment[]) =>
        dispatch({ type: "SET_PENDING_ATTACHMENTS", payload: attachments }),
      [],
    ),
    addPendingAttachment: useCallback(
      (attachment: ImageAttachment) =>
        dispatch({ type: "ADD_PENDING_ATTACHMENT", payload: attachment }),
      [],
    ),
    removePendingAttachment: useCallback(
      (attachmentId: string) =>
        dispatch({ type: "REMOVE_PENDING_ATTACHMENT", payload: attachmentId }),
      [],
    ),
    clearPendingAttachments: useCallback(() => dispatch({ type: "CLEAR_PENDING_ATTACHMENTS" }), []),

    // Actions
    handleSend,
    handleContinue,
    handleRegenerate,
    handleAbort,
    loadOlderMessages,
    ensureMessageLoaded,
    getVariantState,
    applyVariantSelection,
    handleVariantSwipe,
    handleVariantDrag,
    handleSaveEdit,
    handleDeleteMessage,
    handleRewindToMessage,
    handleBranchFromMessage,
    handleBranchToCharacter,
    handleTogglePin,
    resetMessageActions,
    initializeLongPressTimer,
    isStartingSceneMessage: useCallback((message: StoredMessage) => {
      return isStartingSceneMessage(message);
    }, []),
  };
}
