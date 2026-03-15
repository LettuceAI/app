import { useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  continueConversation,
  regenerateAssistantMessage,
  sendChatTurn,
} from "../../../../core/chat/manager";
import { getSessionMeta, listMessages } from "../../../../core/storage/repo";
import type { ImageAttachment, StoredMessage } from "../../../../core/storage/schemas";
import {
  consumeThinkDelta,
  createThinkStreamState,
  finalizeThinkStream,
} from "../../../../core/utils/thinkTags";
import { type ChatControllerPagingContext, isStartingSceneMessage } from "./chatControllerShared";
import { applyLiveChatAction, setLiveChatState } from "./chatLiveState";

const INITIAL_MESSAGE_LIMIT = 50;

function createStreamBatcher(dispatch: ChatControllerPagingContext["dispatch"]) {
  const pendingContentByMessage = new Map<string, string>();
  const messageOrder: string[] = [];
  let rafId: number | null = null;

  const flush = () => {
    if (messageOrder.length > 0) {
      const actions = messageOrder
        .map((messageId) => {
          const content = pendingContentByMessage.get(messageId) ?? "";
          return content
            ? {
                type: "UPDATE_MESSAGE_CONTENT" as const,
                payload: { messageId, content },
              }
            : null;
        })
        .filter(
          (
            action,
          ): action is {
            type: "UPDATE_MESSAGE_CONTENT";
            payload: { messageId: string; content: string };
          } => action !== null,
        );

      if (actions.length === 1) {
        dispatch(actions[0]);
      } else if (actions.length > 1) {
        dispatch({ type: "BATCH", actions });
      }

      pendingContentByMessage.clear();
      messageOrder.length = 0;
    }
    rafId = null;
  };

  return {
    update: (messageId: string, content: string) => {
      if (!content) return;
      if (!pendingContentByMessage.has(messageId)) {
        pendingContentByMessage.set(messageId, content);
        messageOrder.push(messageId);
      } else {
        pendingContentByMessage.set(messageId, pendingContentByMessage.get(messageId)! + content);
      }
      if (rafId === null) {
        rafId = requestAnimationFrame(flush);
      }
    },
    cancel: () => {
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      pendingContentByMessage.clear();
      messageOrder.length = 0;
    },
  };
}

function createPlaceholderMessage(
  role: "user" | "assistant",
  content: string,
  attachments?: ImageAttachment[],
): StoredMessage {
  return {
    id: `placeholder-${role}-${crypto.randomUUID()}`,
    role,
    content,
    createdAt: Date.now(),
    usage: undefined,
    variants: [],
    selectedVariantId: undefined,
    isPinned: false,
    memoryRefs: [],
    attachments: attachments ?? [],
  };
}

interface UseChatStreamingControllerArgs {
  context: ChatControllerPagingContext;
  runInChatImageGeneration: (assistantMessageId: string) => Promise<void> | void;
  reloadSessionStateFromStorage: (sessionId: string) => Promise<void>;
  triggerTypingHaptic: () => Promise<void>;
}

export function useChatStreamingController({
  context,
  runInChatImageGeneration,
  reloadSessionStateFromStorage,
  triggerTypingHaptic,
}: UseChatStreamingControllerArgs) {
  const { state, dispatch, messagesRef, hasMoreMessagesBeforeRef } = context;

  const handleSend = useCallback(
    async (
      message: string,
      attachments?: ImageAttachment[],
      options?: { swapPlaces?: boolean },
    ) => {
      if (!state.session || !state.character) return;
      const currentSessionId = state.session.id;
      const requestId = crypto.randomUUID();
      const messageAttachments = attachments ?? state.pendingAttachments;
      const userPlaceholder = createPlaceholderMessage("user", message, messageAttachments);
      const assistantPlaceholder = createPlaceholderMessage("assistant", "");
      const optimisticMessages = [...state.messages, userPlaceholder, assistantPlaceholder];

      messagesRef.current = optimisticMessages;
      // The attachments are cleared in the state via BATCH/CLEAR_PENDING_ATTACHMENTS.
      // The draft will be cleared only after the send succeeds.

      dispatch({
        type: "BATCH",
        actions: [
          { type: "SET_SENDING", payload: true },
          { type: "SET_ACTIVE_REQUEST_ID", payload: requestId },
          { type: "SET_MESSAGES", payload: optimisticMessages },
          { type: "CLEAR_PENDING_ATTACHMENTS" },
          { type: "CLEAR_DRAFT" },
        ],
      });
      applyLiveChatAction(currentSessionId, state, {
        type: "BATCH",
        actions: [
          { type: "SET_SENDING", payload: true },
          { type: "SET_ACTIVE_REQUEST_ID", payload: requestId },
          { type: "SET_MESSAGES", payload: optimisticMessages },
          { type: "SET_ERROR", payload: null },
          { type: "CLEAR_DRAFT" },
        ],
      });

      let unlistenNormalized: UnlistenFn | null = null;
      const streamBatcher = createStreamBatcher(dispatch);
      const thinkState = createThinkStreamState();

      try {
        unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
          try {
            const payload =
              typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;
            if (payload && payload.type === "delta" && payload.data?.text) {
              const { content, reasoning } = consumeThinkDelta(
                thinkState,
                String(payload.data.text),
              );
              if (content) {
                streamBatcher.update(assistantPlaceholder.id, content);
                applyLiveChatAction(currentSessionId, state, {
                  type: "UPDATE_MESSAGE_CONTENT",
                  payload: { messageId: assistantPlaceholder.id, content },
                });
              }
              if (reasoning) {
                dispatch({
                  type: "UPDATE_MESSAGE_REASONING",
                  payload: { messageId: assistantPlaceholder.id, reasoning },
                });
                applyLiveChatAction(currentSessionId, state, {
                  type: "UPDATE_MESSAGE_REASONING",
                  payload: { messageId: assistantPlaceholder.id, reasoning },
                });
              }
              if (content || reasoning) {
                void triggerTypingHaptic();
              }
            } else if (payload && payload.type === "reasoning" && payload.data?.text) {
              const reasoning = String(payload.data.text);
              dispatch({
                type: "UPDATE_MESSAGE_REASONING",
                payload: { messageId: assistantPlaceholder.id, reasoning },
              });
              applyLiveChatAction(currentSessionId, state, {
                type: "UPDATE_MESSAGE_REASONING",
                payload: { messageId: assistantPlaceholder.id, reasoning },
              });
            } else if (payload && payload.type === "error" && payload.data?.message) {
              const error = String(payload.data.message);
              dispatch({ type: "SET_ERROR", payload: error });
              applyLiveChatAction(currentSessionId, state, {
                type: "SET_ERROR",
                payload: error,
              });
            }
          } catch {
            // ignore malformed payloads
          }
        });

        const result = await sendChatTurn({
          sessionId: state.session.id,
          characterId: state.character.id,
          message,
          personaId: state.persona?.id,
          swapPlaces: options?.swapPlaces ?? false,
          stream: true,
          requestId,
          attachments: messageAttachments.length > 0 ? messageAttachments : undefined,
        });

        const replaced = messagesRef.current.map((msg) => {
          if (msg.id === userPlaceholder.id) return result.userMessage;
          if (msg.id === assistantPlaceholder.id) return result.assistantMessage;
          return msg;
        });
        messagesRef.current = replaced;
        const updatedSession = {
          ...state.session,
          id: result.sessionId,
          updatedAt: result.sessionUpdatedAt,
          messages: replaced,
        };
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: updatedSession },
            { type: "SET_MESSAGES", payload: replaced },
            {
              type: "TRANSFER_REASONING",
              payload: { fromId: assistantPlaceholder.id, toId: result.assistantMessage.id },
            },
          ],
        });
        applyLiveChatAction(currentSessionId, state, {
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: updatedSession },
            { type: "SET_MESSAGES", payload: replaced },
            {
              type: "TRANSFER_REASONING",
              payload: { fromId: assistantPlaceholder.id, toId: result.assistantMessage.id },
            },
          ],
        });
        if (result.assistantMessage.reasoning) {
          dispatch({ type: "CLEAR_STREAMING_REASONING", payload: result.assistantMessage.id });
          applyLiveChatAction(currentSessionId, state, {
            type: "CLEAR_STREAMING_REASONING",
            payload: result.assistantMessage.id,
          });
        }

        void runInChatImageGeneration(result.assistantMessage.id);
      } catch (err) {
        const error = err instanceof Error ? err.message : String(err);
        console.error("ChatStreamingController: send failed", err);
        dispatch({ type: "SET_ERROR", payload: error });
        applyLiveChatAction(currentSessionId, state, { type: "SET_ERROR", payload: error });
        try {
          await reloadSessionStateFromStorage(currentSessionId);
        } catch (reloadErr) {
          console.warn(
            "ChatStreamingController: failed to resync session after send error",
            reloadErr,
          );
          const cleaned = messagesRef.current.filter((msg) => msg.id !== assistantPlaceholder.id);
          messagesRef.current = cleaned;
          dispatch({ type: "SET_MESSAGES", payload: cleaned });
          applyLiveChatAction(currentSessionId, state, {
            type: "SET_MESSAGES",
            payload: cleaned,
          });
        }
      } finally {
        const tail = finalizeThinkStream(thinkState);
        if (tail.content) {
          dispatch({
            type: "UPDATE_MESSAGE_CONTENT",
            payload: { messageId: assistantPlaceholder.id, content: tail.content },
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "UPDATE_MESSAGE_CONTENT",
            payload: { messageId: assistantPlaceholder.id, content: tail.content },
          });
        }
        if (tail.reasoning) {
          dispatch({
            type: "UPDATE_MESSAGE_REASONING",
            payload: { messageId: assistantPlaceholder.id, reasoning: tail.reasoning },
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "UPDATE_MESSAGE_REASONING",
            payload: { messageId: assistantPlaceholder.id, reasoning: tail.reasoning },
          });
        }
        streamBatcher.cancel();
        if (unlistenNormalized) unlistenNormalized();
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_SENDING", payload: false },
            { type: "SET_ACTIVE_REQUEST_ID", payload: null },
          ],
        });
        setLiveChatState(currentSessionId, null);
      }
    },
    [
      dispatch,
      messagesRef,
      reloadSessionStateFromStorage,
      runInChatImageGeneration,
      state,
      triggerTypingHaptic,
    ],
  );

  const handleContinue = useCallback(
    async (options?: { swapPlaces?: boolean }) => {
      if (!state.session || !state.character) return;
      const currentSessionId = state.session.id;
      const requestId = crypto.randomUUID();
      const assistantPlaceholder = createPlaceholderMessage("assistant", "");
      const optimisticMessages = [...state.messages, assistantPlaceholder];

      messagesRef.current = optimisticMessages;

      dispatch({
        type: "BATCH",
        actions: [
          { type: "SET_SENDING", payload: true },
          { type: "SET_ACTIVE_REQUEST_ID", payload: requestId },
          { type: "SET_MESSAGES", payload: optimisticMessages },
        ],
      });
      applyLiveChatAction(currentSessionId, state, {
        type: "BATCH",
        actions: [
          { type: "SET_SENDING", payload: true },
          { type: "SET_ACTIVE_REQUEST_ID", payload: requestId },
          { type: "SET_MESSAGES", payload: optimisticMessages },
          { type: "SET_ERROR", payload: null },
        ],
      });

      let unlistenNormalized: UnlistenFn | null = null;
      const streamBatcher = createStreamBatcher(dispatch);
      const thinkState = createThinkStreamState();

      try {
        unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
          try {
            const payload =
              typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;
            if (payload && payload.type === "delta" && payload.data?.text) {
              const { content, reasoning } = consumeThinkDelta(
                thinkState,
                String(payload.data.text),
              );
              if (content) {
                streamBatcher.update(assistantPlaceholder.id, content);
                applyLiveChatAction(currentSessionId, state, {
                  type: "UPDATE_MESSAGE_CONTENT",
                  payload: { messageId: assistantPlaceholder.id, content },
                });
              }
              if (reasoning) {
                dispatch({
                  type: "UPDATE_MESSAGE_REASONING",
                  payload: { messageId: assistantPlaceholder.id, reasoning },
                });
                applyLiveChatAction(currentSessionId, state, {
                  type: "UPDATE_MESSAGE_REASONING",
                  payload: { messageId: assistantPlaceholder.id, reasoning },
                });
              }
              if (content || reasoning) {
                void triggerTypingHaptic();
              }
            } else if (payload && payload.type === "reasoning" && payload.data?.text) {
              const reasoning = String(payload.data.text);
              dispatch({
                type: "UPDATE_MESSAGE_REASONING",
                payload: { messageId: assistantPlaceholder.id, reasoning },
              });
              applyLiveChatAction(currentSessionId, state, {
                type: "UPDATE_MESSAGE_REASONING",
                payload: { messageId: assistantPlaceholder.id, reasoning },
              });
            } else if (payload && payload.type === "error" && payload.data?.message) {
              const error = String(payload.data.message);
              dispatch({ type: "SET_ERROR", payload: error });
              applyLiveChatAction(currentSessionId, state, {
                type: "SET_ERROR",
                payload: error,
              });
            }
          } catch {
            // ignore malformed payloads
          }
        });

        const result = await continueConversation({
          sessionId: state.session.id,
          characterId: state.character.id,
          personaId: state.persona?.id,
          swapPlaces: options?.swapPlaces ?? false,
          stream: true,
          requestId,
        });

        const replaced = messagesRef.current.map((msg) =>
          msg.id === assistantPlaceholder.id ? result.assistantMessage : msg,
        );
        messagesRef.current = replaced;
        const updatedSession = {
          ...state.session,
          id: result.sessionId,
          updatedAt: result.sessionUpdatedAt,
          messages: replaced,
        };
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: updatedSession },
            { type: "SET_MESSAGES", payload: replaced },
            {
              type: "TRANSFER_REASONING",
              payload: { fromId: assistantPlaceholder.id, toId: result.assistantMessage.id },
            },
          ],
        });
        applyLiveChatAction(currentSessionId, state, {
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: updatedSession },
            { type: "SET_MESSAGES", payload: replaced },
            {
              type: "TRANSFER_REASONING",
              payload: { fromId: assistantPlaceholder.id, toId: result.assistantMessage.id },
            },
          ],
        });
        if (result.assistantMessage.reasoning) {
          dispatch({ type: "CLEAR_STREAMING_REASONING", payload: result.assistantMessage.id });
          applyLiveChatAction(currentSessionId, state, {
            type: "CLEAR_STREAMING_REASONING",
            payload: result.assistantMessage.id,
          });
        }

        void runInChatImageGeneration(result.assistantMessage.id);
      } catch (err) {
        const error = err instanceof Error ? err.message : String(err);
        console.error("ChatStreamingController: continue failed", err);
        dispatch({ type: "SET_ERROR", payload: error });
        applyLiveChatAction(currentSessionId, state, { type: "SET_ERROR", payload: error });

        const abortedByUser =
          error.toLowerCase().includes("aborted by user") ||
          error.toLowerCase().includes("cancelled");
        if (!abortedByUser) {
          const cleaned = messagesRef.current.filter((msg) => msg.id !== assistantPlaceholder.id);
          messagesRef.current = cleaned;
          dispatch({ type: "SET_MESSAGES", payload: cleaned });
          applyLiveChatAction(currentSessionId, state, {
            type: "SET_MESSAGES",
            payload: cleaned,
          });
        }
      } finally {
        const tail = finalizeThinkStream(thinkState);
        if (tail.content) {
          dispatch({
            type: "UPDATE_MESSAGE_CONTENT",
            payload: { messageId: assistantPlaceholder.id, content: tail.content },
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "UPDATE_MESSAGE_CONTENT",
            payload: { messageId: assistantPlaceholder.id, content: tail.content },
          });
        }
        if (tail.reasoning) {
          dispatch({
            type: "UPDATE_MESSAGE_REASONING",
            payload: { messageId: assistantPlaceholder.id, reasoning: tail.reasoning },
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "UPDATE_MESSAGE_REASONING",
            payload: { messageId: assistantPlaceholder.id, reasoning: tail.reasoning },
          });
        }
        streamBatcher.cancel();
        if (unlistenNormalized) unlistenNormalized();
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_SENDING", payload: false },
            { type: "SET_ACTIVE_REQUEST_ID", payload: null },
          ],
        });
        setLiveChatState(currentSessionId, null);
      }
    },
    [dispatch, messagesRef, runInChatImageGeneration, state, triggerTypingHaptic],
  );

  const handleRegenerate = useCallback(
    async (message: StoredMessage, options?: { swapPlaces?: boolean }) => {
      if (!state.session) return;
      const currentSessionId = state.session.id;
      if (
        state.messages.length === 0 ||
        state.messages[state.messages.length - 1]?.id !== message.id
      ) {
        return;
      }
      if (message.role !== "assistant" || state.regeneratingMessageId) return;
      if (isStartingSceneMessage(message)) return;

      const messageInSession = state.messages.find((candidate) => candidate.id === message.id);
      if (!messageInSession) {
        console.error(
          "ChatStreamingController: cannot regenerate - message not found in current messages",
          message.id,
        );
        return;
      }

      const requestId = crypto.randomUUID();
      let unlistenNormalized: UnlistenFn | null = null;
      const regeneratingMessages = state.messages.map((candidate) =>
        candidate.id === message.id
          ? { ...candidate, content: "", reasoning: undefined }
          : candidate,
      );

      dispatch({
        type: "BATCH",
        actions: [
          { type: "SET_REGENERATING_MESSAGE_ID", payload: message.id },
          { type: "SET_ACTIVE_REQUEST_ID", payload: requestId },
          { type: "SET_SENDING", payload: true },
          { type: "SET_ERROR", payload: null },
          { type: "SET_HELD_MESSAGE_ID", payload: null },
          { type: "CLEAR_STREAMING_REASONING", payload: message.id },
          { type: "SET_MESSAGES", payload: regeneratingMessages },
        ],
      });
      messagesRef.current = regeneratingMessages;
      applyLiveChatAction(currentSessionId, state, {
        type: "BATCH",
        actions: [
          { type: "SET_REGENERATING_MESSAGE_ID", payload: message.id },
          { type: "SET_ACTIVE_REQUEST_ID", payload: requestId },
          { type: "SET_SENDING", payload: true },
          { type: "SET_ERROR", payload: null },
          { type: "SET_HELD_MESSAGE_ID", payload: null },
          { type: "CLEAR_STREAMING_REASONING", payload: message.id },
          { type: "SET_MESSAGES", payload: regeneratingMessages },
        ],
      });

      const streamBatcher = createStreamBatcher(dispatch);
      const thinkState = createThinkStreamState();

      try {
        unlistenNormalized = await listen<any>(`api-normalized://${requestId}`, (event) => {
          try {
            const payload =
              typeof event.payload === "string" ? JSON.parse(event.payload) : event.payload;
            if (payload && payload.type === "delta" && payload.data?.text) {
              const { content, reasoning } = consumeThinkDelta(
                thinkState,
                String(payload.data.text),
              );
              if (content) {
                streamBatcher.update(message.id, content);
                applyLiveChatAction(currentSessionId, state, {
                  type: "UPDATE_MESSAGE_CONTENT",
                  payload: { messageId: message.id, content },
                });
              }
              if (reasoning) {
                dispatch({
                  type: "UPDATE_MESSAGE_REASONING",
                  payload: { messageId: message.id, reasoning },
                });
                applyLiveChatAction(currentSessionId, state, {
                  type: "UPDATE_MESSAGE_REASONING",
                  payload: { messageId: message.id, reasoning },
                });
              }
              if (content || reasoning) {
                void triggerTypingHaptic();
              }
            } else if (payload && payload.type === "reasoning" && payload.data?.text) {
              const reasoning = String(payload.data.text);
              dispatch({
                type: "UPDATE_MESSAGE_REASONING",
                payload: { messageId: message.id, reasoning },
              });
              applyLiveChatAction(currentSessionId, state, {
                type: "UPDATE_MESSAGE_REASONING",
                payload: { messageId: message.id, reasoning },
              });
            } else if (payload && payload.type === "error" && payload.data?.message) {
              const error = String(payload.data.message);
              dispatch({ type: "SET_ERROR", payload: error });
              applyLiveChatAction(currentSessionId, state, {
                type: "SET_ERROR",
                payload: error,
              });
            }
          } catch {
            // ignore malformed payloads
          }
        });

        const result = await regenerateAssistantMessage({
          sessionId: state.session.id,
          messageId: message.id,
          swapPlaces: options?.swapPlaces ?? false,
          stream: true,
          requestId,
        });

        const replaced = messagesRef.current.map((candidate) =>
          candidate.id === message.id ? result.assistantMessage : candidate,
        );
        messagesRef.current = replaced;
        const updatedSession = {
          ...state.session,
          id: result.sessionId,
          updatedAt: result.sessionUpdatedAt,
          messages: replaced,
        };
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: updatedSession },
            { type: "SET_MESSAGES", payload: replaced },
          ],
        });
        applyLiveChatAction(currentSessionId, state, {
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: updatedSession },
            { type: "SET_MESSAGES", payload: replaced },
          ],
        });
        if (result.assistantMessage.reasoning) {
          dispatch({ type: "CLEAR_STREAMING_REASONING", payload: result.assistantMessage.id });
          applyLiveChatAction(currentSessionId, state, {
            type: "CLEAR_STREAMING_REASONING",
            payload: result.assistantMessage.id,
          });
        }

        void runInChatImageGeneration(result.assistantMessage.id);

        if (state.messageAction?.message.id === message.id) {
          dispatch({
            type: "SET_MESSAGE_ACTION",
            payload: { message: result.assistantMessage, mode: state.messageAction.mode },
          });
        }
      } catch (err) {
        const error = err instanceof Error ? err.message : String(err);
        console.error("ChatStreamingController: regenerate failed", err);
        dispatch({ type: "SET_ERROR", payload: error });
        applyLiveChatAction(currentSessionId, state, { type: "SET_ERROR", payload: error });
        const meta = await getSessionMeta(state.session.id).catch(() => null);
        const refreshed = await listMessages(state.session.id, {
          limit: Math.max(INITIAL_MESSAGE_LIMIT, messagesRef.current.length),
        }).catch(() => [] as StoredMessage[]);
        const ordered = [...refreshed].sort((a, b) => a.createdAt - b.createdAt);
        messagesRef.current = ordered;
        hasMoreMessagesBeforeRef.current = ordered.length >= INITIAL_MESSAGE_LIMIT;
        if (meta) {
          dispatch({
            type: "BATCH",
            actions: [
              { type: "SET_SESSION", payload: { ...meta, messages: ordered } },
              { type: "SET_MESSAGES", payload: ordered },
            ],
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "BATCH",
            actions: [
              { type: "SET_SESSION", payload: { ...meta, messages: ordered } },
              { type: "SET_MESSAGES", payload: ordered },
            ],
          });
        } else {
          dispatch({ type: "SET_MESSAGES", payload: ordered });
          applyLiveChatAction(currentSessionId, state, {
            type: "SET_MESSAGES",
            payload: ordered,
          });
        }
      } finally {
        const tail = finalizeThinkStream(thinkState);
        if (tail.content) {
          dispatch({
            type: "UPDATE_MESSAGE_CONTENT",
            payload: { messageId: message.id, content: tail.content },
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "UPDATE_MESSAGE_CONTENT",
            payload: { messageId: message.id, content: tail.content },
          });
        }
        if (tail.reasoning) {
          dispatch({
            type: "UPDATE_MESSAGE_REASONING",
            payload: { messageId: message.id, reasoning: tail.reasoning },
          });
          applyLiveChatAction(currentSessionId, state, {
            type: "UPDATE_MESSAGE_REASONING",
            payload: { messageId: message.id, reasoning: tail.reasoning },
          });
        }
        streamBatcher.cancel();
        if (unlistenNormalized) unlistenNormalized();
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_REGENERATING_MESSAGE_ID", payload: null },
            { type: "SET_ACTIVE_REQUEST_ID", payload: null },
            { type: "SET_SENDING", payload: false },
          ],
        });
        setLiveChatState(currentSessionId, null);
      }
    },
    [
      dispatch,
      hasMoreMessagesBeforeRef,
      isStartingSceneMessage,
      messagesRef,
      runInChatImageGeneration,
      state,
      triggerTypingHaptic,
    ],
  );

  return {
    handleSend,
    handleContinue,
    handleRegenerate,
  };
}
