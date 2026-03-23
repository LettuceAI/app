import { useCallback } from "react";

import {
  createBranchedSession,
  createBranchedSessionToCharacter,
  deleteMessage,
  deleteMessagesAfter,
  getSession,
  toggleMessagePin,
} from "../../../../core/storage/repo";
import type { Session, StoredMessage } from "../../../../core/storage/schemas";
import { confirmBottomMenu } from "../../../components/ConfirmBottomMenu";
import {
  type ChatControllerModuleContext,
  isStartingSceneMessage,
  resolveSceneContent,
} from "./chatControllerShared";

export interface VariantState {
  variants: StoredMessage["variants"];
  selectedIndex: number;
  total: number;
}

interface UseChatMessageActionsControllerArgs {
  context: ChatControllerModuleContext;
}

export function useChatMessageActionsController({ context }: UseChatMessageActionsControllerArgs) {
  const { state, dispatch, messagesRef, persistSession } = context;

  const resetMessageActions = useCallback(() => {
    dispatch({ type: "RESET_MESSAGE_ACTIONS" });
  }, [dispatch]);

  const getVariantState = useCallback(
    (message: StoredMessage): VariantState => {
      if (isStartingSceneMessage(message)) {
        if (!state.character || !state.session?.selectedSceneId) {
          return { variants: [], selectedIndex: -1, total: 0 };
        }

        const currentSceneIndex = state.character.scenes.findIndex(
          (scene) => scene.id === state.session!.selectedSceneId,
        );

        return {
          variants: state.character.scenes as any,
          selectedIndex: currentSceneIndex,
          total: state.character.scenes.length,
        };
      }

      const variants = message.variants ?? [];
      if (variants.length === 0) {
        return { variants, selectedIndex: -1, total: 0 };
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
    },
    [isStartingSceneMessage, state.character, state.session],
  );

  const applyVariantSelection = useCallback(
    async (messageId: string, variantId: string) => {
      if (!state.session || state.regeneratingMessageId) return;
      const currentMessage = state.messages.find((message) => message.id === messageId);
      if (!currentMessage) return;

      const variants = currentMessage.variants ?? [];
      const targetVariant = variants.find((variant) => variant.id === variantId);
      if (!targetVariant) return;

      const updatedMessage: StoredMessage = {
        ...currentMessage,
        content: targetVariant.content,
        usage: targetVariant.usage ?? currentMessage.usage,
        reasoning: targetVariant.reasoning,
        selectedVariantId: targetVariant.id,
      };

      const updatedMessages = state.messages.map((message) =>
        message.id === messageId ? updatedMessage : message,
      );
      dispatch({ type: "SET_MESSAGES", payload: updatedMessages });

      const updatedSession: Session = {
        ...state.session,
        messages: updatedMessages,
        updatedAt: Date.now(),
      };
      dispatch({ type: "SET_SESSION", payload: updatedSession });

      if (state.messageAction?.message.id === messageId) {
        dispatch({
          type: "SET_MESSAGE_ACTION",
          payload: { message: updatedMessage, mode: state.messageAction.mode },
        });
      }

      try {
        await persistSession(updatedSession);
      } catch (err) {
        console.error("ChatMessageActionsController: failed to persist variant selection", err);
      }
    },
    [dispatch, persistSession, state],
  );

  const handleVariantSwipe = useCallback(
    async (messageId: string, direction: "prev" | "next") => {
      if (!state.session || state.regeneratingMessageId) return;

      const currentMessage = state.messages.find((message) => message.id === messageId);
      if (!currentMessage) return;

      if (isStartingSceneMessage(currentMessage)) {
        if (!state.character || !state.session?.selectedSceneId) return;

        const currentSceneIndex = state.character.scenes.findIndex(
          (scene) => scene.id === state.session!.selectedSceneId,
        );
        if (currentSceneIndex === -1) return;

        const nextSceneIndex = direction === "next" ? currentSceneIndex + 1 : currentSceneIndex - 1;
        if (nextSceneIndex < 0 || nextSceneIndex >= state.character.scenes.length) return;

        const nextScene = state.character.scenes[nextSceneIndex];
        const sceneContent = resolveSceneContent(nextScene);

        const updatedMessage: StoredMessage = {
          ...currentMessage,
          content: sceneContent,
          sceneEdited: false,
        };

        const updatedMessages = state.messages.map((message) =>
          message.id === messageId ? updatedMessage : message,
        );

        const updatedSession: Session = {
          ...state.session,
          selectedSceneId: nextScene.id,
          messages: updatedMessages,
          updatedAt: Date.now(),
        };

        dispatch({ type: "SET_SESSION", payload: updatedSession });
        dispatch({ type: "SET_MESSAGES", payload: updatedMessages });

        try {
          await persistSession(updatedSession);
        } catch (err) {
          console.error("ChatMessageActionsController: failed to persist scene switch", err);
        }

        return;
      }

      if (currentMessage.role !== "assistant") return;
      if (
        state.messages.length === 0 ||
        state.messages[state.messages.length - 1]?.id !== messageId
      ) {
        return;
      }

      const variants = currentMessage.variants ?? [];
      if (variants.length <= 1) return;

      const variantState = getVariantState(currentMessage);
      const currentIndex =
        variantState.selectedIndex >= 0 ? variantState.selectedIndex : variants.length - 1;
      const nextIndex = direction === "next" ? currentIndex + 1 : currentIndex - 1;
      if (nextIndex < 0 || nextIndex >= variants.length) return;

      await applyVariantSelection(messageId, variants[nextIndex].id);
    },
    [applyVariantSelection, dispatch, getVariantState, persistSession, state],
  );

  const handleVariantDrag = useCallback(
    async (messageId: string, offsetX: number) => {
      if (offsetX > 60) {
        await handleVariantSwipe(messageId, "prev");
      } else if (offsetX < -60) {
        await handleVariantSwipe(messageId, "next");
      }
    },
    [handleVariantSwipe],
  );

  const handleSaveEdit = useCallback(async () => {
    if (!state.session || !state.messageAction) return;

    const updatedContent = state.editDraft.trim();
    if (!updatedContent) {
      dispatch({ type: "SET_ACTION_ERROR", payload: "Message cannot be empty" });
      return;
    }

    dispatch({
      type: "BATCH",
      actions: [
        { type: "SET_ACTION_BUSY", payload: true },
        { type: "SET_ACTION_ERROR", payload: null },
        { type: "SET_ACTION_STATUS", payload: null },
      ],
    });

    try {
      const editedMessageId = state.messageAction.message.id;
      const editedMessageIndex = messagesRef.current.findIndex(
        (message) => message.id === editedMessageId,
      );
      if (editedMessageIndex === -1) {
        throw new Error("Message not found");
      }
      const updatedMessages = messagesRef.current.map((message) =>
        message.id === editedMessageId
          ? {
              ...message,
              content: updatedContent,
              ...(message.role === "scene" ? { sceneEdited: true } : {}),
              variants: (message.variants ?? []).map((variant) =>
                variant.id === (message.selectedVariantId ?? variant.id)
                  ? { ...variant, content: updatedContent }
                  : variant,
              ),
            }
          : message,
      );
      const updatedSession: Session = {
        ...state.session,
        messages: updatedMessages,
        updatedAt: Date.now(),
      };

      await persistSession(updatedSession);
      messagesRef.current = updatedMessages;
      dispatch({ type: "SET_SESSION", payload: updatedSession });
      dispatch({ type: "SET_MESSAGES", payload: updatedMessages });
      resetMessageActions();
    } catch (err) {
      dispatch({
        type: "SET_ACTION_ERROR",
        payload: err instanceof Error ? err.message : String(err),
      });
    } finally {
      dispatch({ type: "SET_ACTION_BUSY", payload: false });
    }
  }, [dispatch, messagesRef, persistSession, resetMessageActions, state]);

  const handleDeleteMessage = useCallback(
    async (message: StoredMessage) => {
      if (!state.session) return;
      if (message.isPinned) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: "Cannot delete pinned message. Unpin it first.",
        });
        return;
      }

      const confirmed = await confirmBottomMenu({
        title: "Delete message?",
        message: "Are you sure you want to delete this message?",
        confirmLabel: "Delete",
        destructive: true,
      });
      if (!confirmed) return;

      dispatch({ type: "SET_ACTION_BUSY", payload: true });
      dispatch({ type: "SET_ACTION_ERROR", payload: null });
      dispatch({ type: "SET_ACTION_STATUS", payload: null });

      try {
        await deleteMessage(state.session.id, message.id);
        const updatedMessages = messagesRef.current.filter(
          (candidate) => candidate.id !== message.id,
        );
        messagesRef.current = updatedMessages;
        dispatch({
          type: "SET_SESSION",
          payload: { ...state.session, messages: updatedMessages, updatedAt: Date.now() },
        });
        dispatch({ type: "SET_MESSAGES", payload: updatedMessages });
        resetMessageActions();
      } catch (err) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: err instanceof Error ? err.message : String(err),
        });
      } finally {
        dispatch({ type: "SET_ACTION_BUSY", payload: false });
      }
    },
    [dispatch, messagesRef, resetMessageActions, state.session],
  );

  const handleRewindToMessage = useCallback(
    async (message: StoredMessage) => {
      if (!state.session) return;

      const messageIndex = messagesRef.current.findIndex(
        (candidate) => candidate.id === message.id,
      );
      if (messageIndex === -1) {
        dispatch({ type: "SET_ACTION_ERROR", payload: "Message not found" });
        return;
      }

      const messagesAfter = messagesRef.current.slice(messageIndex + 1);
      const hasPinnedAfter = messagesAfter.some((candidate) => candidate.isPinned);
      if (hasPinnedAfter) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: "Cannot rewind: there are pinned messages after this point. Unpin them first.",
        });
        return;
      }

      const confirmed = await confirmBottomMenu({
        title: "Rewind conversation?",
        message:
          "Rewind conversation to this message? All messages after this point will be removed.",
        confirmLabel: "Rewind",
        destructive: true,
      });
      if (!confirmed) return;

      dispatch({ type: "SET_ACTION_BUSY", payload: true });
      dispatch({ type: "SET_ACTION_ERROR", payload: null });
      dispatch({ type: "SET_ACTION_STATUS", payload: null });

      try {
        await deleteMessagesAfter(state.session.id, message.id);
        const updatedMessages = messagesRef.current.slice(0, messageIndex + 1);
        messagesRef.current = updatedMessages;
        dispatch({
          type: "SET_SESSION",
          payload: { ...state.session, messages: updatedMessages, updatedAt: Date.now() },
        });
        dispatch({
          type: "REWIND_TO_MESSAGE",
          payload: { messageId: message.id, messages: updatedMessages },
        });
        resetMessageActions();
      } catch (err) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: err instanceof Error ? err.message : String(err),
        });
      } finally {
        dispatch({ type: "SET_ACTION_BUSY", payload: false });
      }
    },
    [dispatch, messagesRef, resetMessageActions, state.session],
  );

  const handleTogglePin = useCallback(
    async (message: StoredMessage) => {
      if (!state.session) return;

      dispatch({ type: "SET_ACTION_BUSY", payload: true });
      dispatch({ type: "SET_ACTION_ERROR", payload: null });
      dispatch({ type: "SET_ACTION_STATUS", payload: null });

      try {
        const nextPinned = await toggleMessagePin(state.session.id, message.id);

        if (nextPinned !== null) {
          const updatedMessages = messagesRef.current.map((candidate) =>
            candidate.id === message.id ? { ...candidate, isPinned: nextPinned } : candidate,
          );
          messagesRef.current = updatedMessages;
          dispatch({
            type: "SET_SESSION",
            payload: { ...state.session, messages: updatedMessages, updatedAt: Date.now() },
          });
          dispatch({ type: "SET_MESSAGES", payload: updatedMessages });
          dispatch({
            type: "SET_ACTION_STATUS",
            payload: nextPinned ? "Message pinned" : "Message unpinned",
          });
          setTimeout(() => {
            resetMessageActions();
          }, 1000);
        } else {
          dispatch({ type: "SET_ACTION_ERROR", payload: "Failed to toggle pin" });
        }
      } catch (err) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: err instanceof Error ? err.message : String(err),
        });
      } finally {
        dispatch({ type: "SET_ACTION_BUSY", payload: false });
      }
    },
    [dispatch, messagesRef, resetMessageActions, state.session],
  );

  const handleBranchFromMessage = useCallback(
    async (message: StoredMessage): Promise<string | null> => {
      if (!state.session) return null;

      dispatch({ type: "SET_ACTION_BUSY", payload: true });
      dispatch({ type: "SET_ACTION_ERROR", payload: null });
      dispatch({ type: "SET_ACTION_STATUS", payload: null });

      try {
        const fullSession = await getSession(state.session.id);
        if (!fullSession) {
          dispatch({
            type: "SET_ACTION_ERROR",
            payload: "Failed to load full session for branching",
          });
          return null;
        }

        const messageIndex = fullSession.messages.findIndex(
          (candidate) => candidate.id === message.id,
        );
        if (messageIndex === -1) {
          dispatch({ type: "SET_ACTION_ERROR", payload: "Message not found" });
          return null;
        }

        const messageCount = messageIndex + 1;
        const confirmed = await confirmBottomMenu({
          title: "Create chat branch?",
          message: `Create a new chat branch from this point? The new chat will contain ${messageCount} message${messageCount > 1 ? "s" : ""}.`,
          confirmLabel: "Create",
        });
        if (!confirmed) {
          dispatch({ type: "SET_ACTION_BUSY", payload: false });
          return null;
        }

        const branchedSession = await createBranchedSession(fullSession, message.id);
        dispatch({ type: "SET_ACTION_STATUS", payload: "Chat branch created! Redirecting..." });
        setTimeout(() => {
          resetMessageActions();
        }, 500);
        return branchedSession.id;
      } catch (err) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: err instanceof Error ? err.message : String(err),
        });
        return null;
      } finally {
        dispatch({ type: "SET_ACTION_BUSY", payload: false });
      }
    },
    [dispatch, resetMessageActions, state.session],
  );

  const handleBranchToCharacter = useCallback(
    async (
      message: StoredMessage,
      targetCharacterId: string,
    ): Promise<{ sessionId: string; characterId: string } | null> => {
      if (!state.session) return null;

      dispatch({ type: "SET_ACTION_BUSY", payload: true });
      dispatch({ type: "SET_ACTION_ERROR", payload: null });
      dispatch({ type: "SET_ACTION_STATUS", payload: null });

      try {
        const fullSession = await getSession(state.session.id);
        if (!fullSession) {
          dispatch({
            type: "SET_ACTION_ERROR",
            payload: "Failed to load full session for branching",
          });
          return null;
        }

        const messageIndex = fullSession.messages.findIndex(
          (candidate) => candidate.id === message.id,
        );
        if (messageIndex === -1) {
          dispatch({ type: "SET_ACTION_ERROR", payload: "Message not found" });
          return null;
        }

        const branchedSession = await createBranchedSessionToCharacter(
          fullSession,
          message.id,
          targetCharacterId,
        );

        dispatch({ type: "SET_ACTION_STATUS", payload: "Chat branch created! Redirecting..." });
        setTimeout(() => {
          resetMessageActions();
        }, 500);

        return { sessionId: branchedSession.id, characterId: targetCharacterId };
      } catch (err) {
        dispatch({
          type: "SET_ACTION_ERROR",
          payload: err instanceof Error ? err.message : String(err),
        });
        return null;
      } finally {
        dispatch({ type: "SET_ACTION_BUSY", payload: false });
      }
    },
    [dispatch, resetMessageActions, state.session],
  );

  return {
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
  };
}
