import { useCallback, useEffect } from "react";

import {
  createSession,
  getDefaultPersona,
  getSessionMeta,
  listCharacters,
  listMessages,
  listPersonas,
  listSessionPreviews,
  SESSION_UPDATED_EVENT,
  SETTINGS_UPDATED_EVENT,
} from "../../../../core/storage/repo";
import type { Persona, Session, StoredMessage } from "../../../../core/storage/schemas";
import {
  type ChatControllerPagingContext,
  normalizeStartingSceneMessage,
} from "./chatControllerShared";
import { getLiveChatState, setLiveChatState, subscribeToLiveChatState } from "./chatLiveState";
import type { ChatState } from "./chatReducer";

const INITIAL_MESSAGE_LIMIT = 50;
const OLDER_MESSAGE_PAGE = 50;

function sortMessages(messages: StoredMessage[]): StoredMessage[] {
  return [...messages].sort((left, right) =>
    left.createdAt === right.createdAt
      ? left.id.localeCompare(right.id)
      : left.createdAt - right.createdAt,
  );
}

function hasTransientLiveSnapshot(snapshot: ChatState): boolean {
  return (
    snapshot.sending ||
    Boolean(snapshot.activeRequestId) ||
    Boolean(snapshot.regeneratingMessageId) ||
    snapshot.messages.some((message) => message.id.startsWith("placeholder-"))
  );
}

interface UseChatSessionControllerArgs {
  context: ChatControllerPagingContext;
  characterId?: string;
  sessionId?: string;
}

export function useChatSessionController({
  context,
  characterId,
  sessionId,
}: UseChatSessionControllerArgs) {
  const {
    state,
    dispatch,
    messagesRef,
    hasMoreMessagesBeforeRef,
    loadingOlderRef,
    sessionOperationRef,
    log,
    recordSessionTimestamp,
  } = context;

  useEffect(() => {
    if (typeof window === "undefined") return;

    const handler = async () => {
      if (sessionOperationRef.current) {
        log.info("Skipping settings reload - session operation in progress");
        return;
      }

      if (characterId && state.character) {
        try {
          const list = await listCharacters();
          const match = list.find((character) => character.id === characterId) ?? null;
          if (match) {
            dispatch({ type: "SET_CHARACTER", payload: match });
          }
        } catch (err) {
          log.error("Failed to reload character on settings change", err);
        }
      }
    };

    window.addEventListener(SETTINGS_UPDATED_EVENT, handler);
    return () => {
      window.removeEventListener(SETTINGS_UPDATED_EVENT, handler);
    };
  }, [characterId, dispatch, log, sessionOperationRef, state.character]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (!state.session?.id) return;

    let cancelled = false;

    const handler = async () => {
      try {
        const latest = await getSessionMeta(state.session!.id);
        if (!latest || cancelled) return;
        if (state.character && latest.characterId !== state.character.id) return;

        const currentUpdatedAt = state.session?.updatedAt ?? 0;
        if (latest.updatedAt <= currentUpdatedAt) {
          return;
        }

        const messageLimit = Math.max(
          INITIAL_MESSAGE_LIMIT,
          messagesRef.current.filter((message) => !message.id.startsWith("placeholder-")).length,
        );
        const storedMessages = await listMessages(state.session!.id, {
          limit: messageLimit,
        }).catch((err) => {
          console.warn("ChatSessionController: failed to reload messages after session update", err);
          return [] as StoredMessage[];
        });
        let orderedMessages = sortMessages(storedMessages);
        if (state.character) {
          orderedMessages = normalizeStartingSceneMessage(
            orderedMessages,
            state.character,
            latest.selectedSceneId,
          );
        }
        messagesRef.current = orderedMessages;

        const normalizedSession: Session = {
          ...latest,
          messages: orderedMessages,
        };
        const personaDisabled =
          normalizedSession.personaDisabled || normalizedSession.personaId === "";
        let selectedPersona: Persona | null = null;
        if (!personaDisabled && normalizedSession.personaId) {
          const allPersonas = await listPersonas().catch(() => [] as Persona[]);
          selectedPersona =
            allPersonas.find((persona) => persona.id === normalizedSession.personaId) ?? null;
        }
        if (!selectedPersona && !personaDisabled) {
          selectedPersona = await getDefaultPersona().catch((err) => {
            console.warn("ChatSessionController: failed to load persona", err);
            return null;
          });
        }

        if (cancelled) return;
        recordSessionTimestamp(normalizedSession.updatedAt);
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_PERSONA", payload: selectedPersona },
            { type: "SET_SESSION", payload: normalizedSession },
            { type: "SET_MESSAGES", payload: orderedMessages },
          ],
        });
      } catch (err) {
        console.warn("ChatSessionController: failed to sync session update", err);
      }
    };

    window.addEventListener(SESSION_UPDATED_EVENT, handler);
    return () => {
      cancelled = true;
      window.removeEventListener(SESSION_UPDATED_EVENT, handler);
    };
  }, [dispatch, messagesRef, recordSessionTimestamp, state.character, state.session?.id]);

  useEffect(() => {
    if (!characterId) return;

    let cancelled = false;

    (async () => {
      try {
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_LOADING", payload: true },
            { type: "SET_ERROR", payload: null },
          ],
        });

        const list = await listCharacters();
        const match = list.find((character) => character.id === characterId) ?? null;
        if (!match) {
          if (!cancelled) {
            dispatch({ type: "SET_CHARACTER", payload: null });
          }
          return;
        }

        let targetSession: Session | null = null;

        if (sessionId) {
          const explicitSession = await getSessionMeta(sessionId).catch((err) => {
            console.warn("ChatSessionController: failed to load requested session", {
              sessionId,
              err,
            });
            return null;
          });
          if (explicitSession && explicitSession.characterId === match.id) {
            targetSession = explicitSession;
          }
        }

        if (!targetSession) {
          const previews = await listSessionPreviews(match.id, 1).catch(() => []);
          const latestId = previews[0]?.id;
          if (latestId) {
            targetSession = await getSessionMeta(latestId).catch((err) => {
              console.warn("ChatSessionController: failed to load latest session", {
                latestId,
                err,
              });
              return null;
            });
          }
        }

        if (!targetSession) {
          targetSession = await createSession(
            match.id,
            match.name ?? "New chat",
            match.defaultSceneId ?? match.scenes?.[0]?.id,
          );
        }

        let orderedMessages: StoredMessage[] = [];
        if (targetSession.messages && targetSession.messages.length > 0) {
          orderedMessages = [...targetSession.messages].sort((a, b) => a.createdAt - b.createdAt);
          hasMoreMessagesBeforeRef.current = false;
        } else {
          const fetched = await listMessages(targetSession.id, {
            limit: INITIAL_MESSAGE_LIMIT,
          }).catch((err) => {
            console.warn("ChatSessionController: failed to load recent messages", {
              sessionId: targetSession?.id,
              err,
            });
            return [] as StoredMessage[];
          });
          orderedMessages = [...fetched].sort((a, b) => a.createdAt - b.createdAt);
          hasMoreMessagesBeforeRef.current = orderedMessages.length >= INITIAL_MESSAGE_LIMIT;
        }

        orderedMessages = normalizeStartingSceneMessage(
          orderedMessages,
          match,
          targetSession.selectedSceneId,
        );

        messagesRef.current = orderedMessages;
        const normalizedSession: Session = { ...targetSession, messages: orderedMessages };

        const personaDisabled =
          normalizedSession.personaDisabled || normalizedSession.personaId === "";
        let selectedPersona: Persona | null = null;
        if (!personaDisabled && normalizedSession.personaId) {
          const allPersonas = await listPersonas().catch(() => [] as Persona[]);
          selectedPersona =
            allPersonas.find((persona) => persona.id === normalizedSession.personaId) ?? null;
        }
        if (!selectedPersona && !personaDisabled) {
          selectedPersona = await getDefaultPersona().catch((err) => {
            console.warn("ChatSessionController: failed to load persona", err);
            return null;
          });
        }

        if (!cancelled) {
          const rawSavedDraft = localStorage.getItem(`chat_draft_${normalizedSession.id}`);
          let savedDraft = "";
          if (rawSavedDraft) {
            try {
              const parsed = JSON.parse(rawSavedDraft);
              savedDraft = parsed.text || "";
            } catch (e) {
              // Legacy format or corrupted
              savedDraft = rawSavedDraft;
            }
          }

          recordSessionTimestamp(normalizedSession.updatedAt);
          dispatch({
            type: "BATCH",
            actions: [
              { type: "SET_CHARACTER", payload: match },
              { type: "SET_PERSONA", payload: selectedPersona },
              { type: "SET_SESSION", payload: normalizedSession },
              { type: "SET_MESSAGES", payload: orderedMessages },
              { type: "SET_DRAFT", payload: savedDraft ?? "" },
            ],
          });
        }
      } catch (err) {
        console.error("ChatSessionController: failed to load chat", err);
        if (!cancelled) {
          dispatch({
            type: "SET_ERROR",
            payload: err instanceof Error ? err.message : String(err),
          });
        }
      } finally {
        if (!cancelled) {
          dispatch({ type: "SET_LOADING", payload: false });
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    characterId,
    dispatch,
    hasMoreMessagesBeforeRef,
    messagesRef,
    recordSessionTimestamp,
    sessionId,
  ]);

  useEffect(() => {
    messagesRef.current = state.messages;
  }, [messagesRef, state.messages]);

  useEffect(() => {
    const liveSessionId = state.session?.id ?? sessionId;
    if (!liveSessionId) return;

    const applySnapshot = (snapshot: ChatState | null) => {
      if (!snapshot) return;
      const loadedUpdatedAt = state.session?.updatedAt ?? 0;
      const snapshotUpdatedAt = snapshot.session?.updatedAt ?? 0;
      const transientSnapshot = hasTransientLiveSnapshot(snapshot);

      if (snapshotUpdatedAt < loadedUpdatedAt) {
        setLiveChatState(liveSessionId, null);
        return;
      }

      if (snapshotUpdatedAt === loadedUpdatedAt && !transientSnapshot) {
        setLiveChatState(liveSessionId, null);
        return;
      }

      messagesRef.current = snapshot.messages;
      dispatch({
        type: "BATCH",
        actions: [
          ...(snapshot.session
            ? [{ type: "SET_SESSION" as const, payload: snapshot.session }]
            : []),
          { type: "SET_MESSAGES", payload: snapshot.messages },
          { type: "SET_SENDING", payload: snapshot.sending },
          { type: "SET_ERROR", payload: snapshot.error },
          { type: "SET_REGENERATING_MESSAGE_ID", payload: snapshot.regeneratingMessageId },
          { type: "SET_ACTIVE_REQUEST_ID", payload: snapshot.activeRequestId },
          { type: "SET_STREAMING_REASONING", payload: snapshot.streamingReasoning },
        ],
      });
    };

    applySnapshot(getLiveChatState(liveSessionId) ?? null);
    return subscribeToLiveChatState(liveSessionId, applySnapshot);
  }, [dispatch, messagesRef, sessionId, state.session?.id, state.session?.updatedAt]);

  const reloadSessionStateFromStorage = useCallback(
    async (targetSessionId: string) => {
      const limit = Math.max(
        INITIAL_MESSAGE_LIMIT,
        messagesRef.current.filter((message) => !message.id.startsWith("placeholder-")).length,
      );
      const [meta, storedMessages] = await Promise.all([
        getSessionMeta(targetSessionId).catch(() => null),
        listMessages(targetSessionId, { limit }).catch(() => [] as StoredMessage[]),
      ]);

      const orderedMessagesSource =
        storedMessages.length > 0 ? storedMessages : (meta?.messages ?? []);
      let orderedMessages = [...orderedMessagesSource].sort((a, b) => a.createdAt - b.createdAt);
      if (state.character) {
        orderedMessages = normalizeStartingSceneMessage(
          orderedMessages,
          state.character,
          meta?.selectedSceneId ?? state.session?.selectedSceneId,
        );
      }

      messagesRef.current = orderedMessages;
      hasMoreMessagesBeforeRef.current = orderedMessages.length >= limit;

      if (meta) {
        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_SESSION", payload: { ...meta, messages: orderedMessages } },
            { type: "SET_MESSAGES", payload: orderedMessages },
          ],
        });
        return;
      }

      dispatch({ type: "SET_MESSAGES", payload: orderedMessages });
    },
    [
      dispatch,
      hasMoreMessagesBeforeRef,
      messagesRef,
      normalizeStartingSceneMessage,
      state.character,
      state.session?.selectedSceneId,
    ],
  );

  const loadOlderMessages = useCallback(async () => {
    if (!state.session) return;
    if (!hasMoreMessagesBeforeRef.current) return;
    if (loadingOlderRef.current) return;
    const first = messagesRef.current[0];
    if (!first) return;

    loadingOlderRef.current = true;
    try {
      const older = await listMessages(state.session.id, {
        limit: OLDER_MESSAGE_PAGE,
        before: { createdAt: first.createdAt, id: first.id },
      });
      if (older.length === 0) {
        hasMoreMessagesBeforeRef.current = false;
        return;
      }

      const merged = [...older, ...messagesRef.current];
      const seen = new Set<string>();
      const deduped = merged.filter((message) => {
        if (seen.has(message.id)) return false;
        seen.add(message.id);
        return true;
      });
      messagesRef.current = deduped;
      hasMoreMessagesBeforeRef.current = older.length >= OLDER_MESSAGE_PAGE;
      dispatch({ type: "SET_MESSAGES", payload: deduped });
      dispatch({ type: "SET_SESSION", payload: { ...state.session, messages: deduped } });
    } catch (err) {
      console.warn("ChatSessionController: failed to load older messages", err);
    } finally {
      loadingOlderRef.current = false;
    }
  }, [dispatch, hasMoreMessagesBeforeRef, loadingOlderRef, messagesRef, state.session]);

  const ensureMessageLoaded = useCallback(
    async (messageId: string) => {
      const maxPages = 20;
      for (let index = 0; index < maxPages; index += 1) {
        if (messagesRef.current.some((message) => message.id === messageId)) return;
        if (!hasMoreMessagesBeforeRef.current) return;
        await loadOlderMessages();
      }
    },
    [hasMoreMessagesBeforeRef, loadOlderMessages, messagesRef],
  );

  return {
    reloadSessionStateFromStorage,
    loadOlderMessages,
    ensureMessageLoaded,
  };
}
