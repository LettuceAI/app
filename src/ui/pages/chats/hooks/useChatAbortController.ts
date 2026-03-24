import { useCallback } from "react";

import { abortMessage } from "../../../../core/chat/manager";
import type { StoredMessage } from "../../../../core/storage/schemas";
import { applyLiveChatAction } from "./chatLiveState";
import type { ChatControllerModuleContext } from "./chatControllerShared";

interface UseChatAbortControllerArgs {
  context: ChatControllerModuleContext;
  reloadSessionStateFromStorage: (sessionId: string) => Promise<void>;
}

function removePlaceholderMessages(messages: StoredMessage[]): StoredMessage[] {
  return messages.filter((message) => !message.id.startsWith("placeholder-"));
}

export function useChatAbortController({
  context,
  reloadSessionStateFromStorage,
}: UseChatAbortControllerArgs) {
  const { state, dispatch, messagesRef, abortedRequestIdsRef, log } = context;

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
    (messages: StoredMessage[]) => {
      if (!state.session) return;
      applyLiveChatAction(state.session.id, state, {
        type: "BATCH",
        actions: [
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
    abortedRequestIdsRef.current.add(requestId);

    releaseLiveRequestOwnership();

    try {
      await abortMessage(requestId);
      log.info("aborted request", requestId);
    } catch (error) {
      log.error("abort failed", error);
    }

    try {
      await reloadSessionStateFromStorage(state.session.id);
      syncLiveStateAfterAbort(messagesRef.current);
    } catch (reloadError) {
      log.error("failed to reload session after abort", reloadError);
      const cleanedMessages = removePlaceholderMessages(messagesRef.current);
      messagesRef.current = cleanedMessages;
      dispatch({ type: "SET_MESSAGES", payload: cleanedMessages });
      syncLiveStateAfterAbort(cleanedMessages);
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
    releaseLiveRequestOwnership,
    reloadSessionStateFromStorage,
    state,
    syncLiveStateAfterAbort,
  ]);

  return { handleAbort };
}
