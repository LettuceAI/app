import { useCallback, useEffect, useReducer, useRef } from "react";

import {
  engineChatReducer,
  initialEngineChatState,
  type EngineChatMessage,
} from "./engineChatReducer";
import { engineChat, engineChatHistory } from "../../../../core/engine/api";
import type { EngineChatResponse, EngineHistoryMessage } from "../../../../core/engine/types";
import { getDefaultPersona } from "../../../../core/storage/repo";

function getUserId(): string {
  const key = "engine_user_id";
  let id = localStorage.getItem(key);
  if (!id) {
    id = crypto.randomUUID();
    localStorage.setItem(key, id);
  }
  return id;
}

export function useEngineChatController(
  baseUrl: string,
  apiKey: string,
  slug: string,
) {
  const [state, dispatch] = useReducer(engineChatReducer, initialEngineChatState);

  const userRef = useRef<{
    userId: string;
    userName: string;
    userDescription: string;
  }>({ userId: getUserId(), userName: "User", userDescription: "" });

  // Load persona + history on mount
  useEffect(() => {
    let cancelled = false;

    (async () => {
      // Resolve user identity from default persona
      try {
        const persona = await getDefaultPersona();
        if (persona && !cancelled) {
          userRef.current = {
            userId: persona.id,
            userName: persona.title,
            userDescription: persona.description,
          };
        }
      } catch {
        // Non-critical — use defaults
      }

      // Load history
      try {
        const raw = await engineChatHistory(baseUrl, apiKey, slug, userRef.current.userId, 50);
        if (cancelled) return;
        const history = raw as EngineHistoryMessage[];
        if (Array.isArray(history)) {
          const messages: EngineChatMessage[] = history
            .filter((m) => m.role === "user" || m.role === "assistant")
            .map((m) => ({
              id: m.id || crypto.randomUUID(),
              role: m.role,
              content: m.content,
              timestamp: m.timestamp,
            }));
          dispatch({ type: "set_messages", payload: messages });
        }
      } catch {
        // No history or endpoint not available — start fresh
      } finally {
        if (!cancelled) dispatch({ type: "set_loading", payload: false });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [baseUrl, apiKey, slug]);

  const sendMessage = useCallback(async () => {
    const text = state.draft.trim();
    if (!text || state.sending) return;

    dispatch({ type: "set_draft", payload: "" });
    dispatch({ type: "set_error", payload: null });

    // Optimistically append user message
    const userMsg: EngineChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: text,
      timestamp: new Date().toISOString(),
    };
    dispatch({ type: "append_message", payload: userMsg });
    dispatch({ type: "set_sending", payload: true });

    try {
      const { userId, userName, userDescription } = userRef.current;
      const result = (await engineChat(baseUrl, apiKey, slug, {
        message: text,
        user_id: userId,
        user_name: userName,
        user_description: userDescription,
      })) as EngineChatResponse;

      const assistantMsg: EngineChatMessage = {
        id: crypto.randomUUID(),
        role: "assistant",
        content: result.response,
        emotion: result.emotion,
        emotionIntensity: result.emotion_intensity,
        timestamp: new Date().toISOString(),
      };
      dispatch({ type: "append_message", payload: assistantMsg });
    } catch (err) {
      dispatch({
        type: "set_error",
        payload: typeof err === "string" ? err : err instanceof Error ? err.message : "Failed to send message.",
      });
    } finally {
      dispatch({ type: "set_sending", payload: false });
    }
  }, [baseUrl, apiKey, slug, state.draft, state.sending]);

  return {
    state,
    setDraft: useCallback((text: string) => dispatch({ type: "set_draft", payload: text }), []),
    sendMessage,
  };
}
