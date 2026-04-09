import { useCallback } from "react";

import { abortMessage } from "../../../../core/chat/manager";
import { getSessionMeta, listMessages } from "../../../../core/storage/repo";
import type { Session, StoredMessage } from "../../../../core/storage/schemas";
import { applyLiveChatAction, getLiveChatState } from "./chatLiveState";
import type { ChatControllerModuleContext } from "./chatControllerShared";
import type { ChatState } from "./chatReducer";

interface UseChatAbortControllerArgs {
  context: ChatControllerModuleContext;
}

const INITIAL_MESSAGE_LIMIT = 50;

function removePlaceholderMessages(messages: StoredMessage[]): StoredMessage[] {
  return messages.filter((message) => !message.id.startsWith("placeholder-"));
}

function sortMessages(messages: StoredMessage[]): StoredMessage[] {
  return [...messages].sort((left, right) =>
    left.createdAt === right.createdAt
      ? left.id.localeCompare(right.id)
      : left.createdAt - right.createdAt,
  );
}

function hasPersistableAssistantContent(message: StoredMessage | undefined): message is StoredMessage {
  return Boolean(message?.role === "assistant" && message.content.trim().length > 0);
}

function buildPartialAssistantMessage(
  message: StoredMessage,
  options?: { reasoning?: string | null; preserveId?: boolean },
): StoredMessage {
  const reasoning = options?.reasoning ?? message.reasoning ?? null;
  const variantId = crypto.randomUUID();
  const partialVariant = {
    id: variantId,
    content: message.content,
    createdAt: Date.now(),
    usage: message.usage ?? null,
    attachments: [...(message.attachments ?? [])],
    reasoning,
  };

  if (options?.preserveId) {
    return {
      ...message,
      reasoning,
      variants: [...(message.variants ?? []), partialVariant],
      selectedVariantId: partialVariant.id,
      attachments: [...(message.attachments ?? [])],
    };
  }

  return {
    ...message,
    id: crypto.randomUUID(),
    reasoning,
    usage: message.usage ?? null,
    variants: [partialVariant],
    selectedVariantId: partialVariant.id,
    attachments: [...(message.attachments ?? [])],
  };
}

function getPartialAssistantForAbort(
  snapshot: ChatState,
): { mode: "append" | "replace"; message: StoredMessage } | null {
  if (snapshot.regeneratingMessageId) {
    const regeneratingMessage = snapshot.messages.find(
      (message) => message.id === snapshot.regeneratingMessageId,
    );
    if (!hasPersistableAssistantContent(regeneratingMessage)) {
      return null;
    }

    return {
      mode: "replace",
      message: buildPartialAssistantMessage(regeneratingMessage, {
        reasoning:
          snapshot.streamingReasoning[regeneratingMessage.id] ?? regeneratingMessage.reasoning,
        preserveId: true,
      }),
    };
  }

  const latestAssistant = [...snapshot.messages]
    .reverse()
    .find((message) => message.role === "assistant" && message.id.startsWith("placeholder-"));
  if (!hasPersistableAssistantContent(latestAssistant)) {
    return null;
  }

  return {
    mode: "append",
    message: buildPartialAssistantMessage(latestAssistant, {
      reasoning: snapshot.streamingReasoning[latestAssistant.id] ?? latestAssistant.reasoning,
    }),
  };
}

function mergePartialAssistantMessage(
  storedMessages: StoredMessage[],
  partialAssistant: { mode: "append" | "replace"; message: StoredMessage },
): StoredMessage[] {
  if (partialAssistant.mode === "replace") {
    const replaced = storedMessages.map((message) =>
      message.id === partialAssistant.message.id ? partialAssistant.message : message,
    );
    const hasReplacement = replaced.some((message) => message.id === partialAssistant.message.id);
    return sortMessages(
      hasReplacement ? replaced : [...storedMessages, partialAssistant.message],
    );
  }

  return sortMessages([...storedMessages, partialAssistant.message]);
}

export function useChatAbortController({
  context,
}: UseChatAbortControllerArgs) {
  const { state, dispatch, messagesRef, abortedRequestIdsRef, log, persistSession } = context;

  const releaseLiveRequestOwnership = useCallback(() => {
    if (!state.session) return;
    applyLiveChatAction(state.session.id, state, {
      type: "BATCH",
      actions: [
        { type: "SET_SENDING", payload: false },
        { type: "SET_REGENERATING_MESSAGE_ID", payload: null },
        { type: "SET_ACTIVE_REQUEST_ID", payload: null },
      ],
    });
  }, [state]);

  const syncLiveStateAfterAbort = useCallback(
    (messages: StoredMessage[], session: Session | null) => {
      if (!state.session) return;
      applyLiveChatAction(state.session.id, state, {
        type: "BATCH",
        actions: [
          ...(session ? ([{ type: "SET_SESSION", payload: session }] as const) : []),
          { type: "SET_MESSAGES", payload: messages },
          { type: "SET_SENDING", payload: false },
          { type: "SET_REGENERATING_MESSAGE_ID", payload: null },
          { type: "SET_ACTIVE_REQUEST_ID", payload: null },
        ],
      });
    },
    [state],
  );

  const handleAbort = useCallback(async () => {
    if (!state.activeRequestId || !state.session) return;
    const requestId = state.activeRequestId;
    const liveSnapshot = getLiveChatState(state.session.id) ?? state;
    const partialAssistant = getPartialAssistantForAbort(liveSnapshot);
    abortedRequestIdsRef.current.add(requestId);

    releaseLiveRequestOwnership();

    try {
      await abortMessage(requestId);
      log.info("aborted request", requestId);
    } catch (error) {
      log.error("abort failed", error);
    }

    try {
      const messageLimit = Math.max(
        INITIAL_MESSAGE_LIMIT,
        liveSnapshot.messages.filter((message) => !message.id.startsWith("placeholder-")).length,
      );
      const [storedSession, storedMessagesRaw] = await Promise.all([
        getSessionMeta(state.session.id).catch(() => null),
        listMessages(state.session.id, { limit: messageLimit }).catch(() => [] as StoredMessage[]),
      ]);

      const storedMessagesSource =
        storedMessagesRaw.length > 0 ? storedMessagesRaw : (storedSession?.messages ?? []);
      let nextMessages = sortMessages(storedMessagesSource);
      let nextSession = storedSession ? { ...storedSession, messages: nextMessages } : null;

      if (storedSession && partialAssistant) {
        const mergedMessages = mergePartialAssistantMessage(nextMessages, partialAssistant);
        const persistedSession = {
          ...storedSession,
          messages: mergedMessages,
          updatedAt: Date.now(),
        };

        try {
          await persistSession(persistedSession);
          nextMessages = mergedMessages;
          nextSession = persistedSession;
        } catch (persistError) {
          log.error("failed to persist partial assistant after abort", persistError);
        }
      }

      messagesRef.current = nextMessages;
      dispatch({
        type: "BATCH",
        actions: [
          ...(nextSession ? ([{ type: "SET_SESSION", payload: nextSession }] as const) : []),
          { type: "SET_MESSAGES", payload: nextMessages },
        ],
      });
      syncLiveStateAfterAbort(nextMessages, nextSession);
    } catch (reloadError) {
      log.error("failed to reload session after abort", reloadError);
      const cleanedMessages = removePlaceholderMessages(messagesRef.current);
      messagesRef.current = cleanedMessages;
      dispatch({ type: "SET_MESSAGES", payload: cleanedMessages });
      syncLiveStateAfterAbort(cleanedMessages, state.session);
    }

    dispatch({
      type: "BATCH",
      actions: [
        { type: "SET_SENDING", payload: false },
        { type: "SET_REGENERATING_MESSAGE_ID", payload: null },
        { type: "SET_ACTIVE_REQUEST_ID", payload: null },
      ],
    });
  }, [
    dispatch,
    log,
    messagesRef,
    abortedRequestIdsRef,
    persistSession,
    releaseLiveRequestOwnership,
    state,
    syncLiveStateAfterAbort,
  ]);

  return { handleAbort };
}
