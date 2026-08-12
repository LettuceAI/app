import { chatReducer, type ChatAction, type ChatState } from "./chatReducer";

const liveChatStateCache = new Map<string, ChatState>();
const liveChatSubscribers = new Map<string, Set<(state: ChatState | null) => void>>();

export function getLiveChatState(sessionId: string): ChatState | undefined {
  return liveChatStateCache.get(sessionId);
}

export function setLiveChatState(sessionId: string, state: ChatState | null) {
  if (state) {
    liveChatStateCache.set(sessionId, state);
  } else {
    liveChatStateCache.delete(sessionId);
  }

  const subscribers = liveChatSubscribers.get(sessionId);
  if (!subscribers || subscribers.size === 0) {
    if (!state) {
      liveChatSubscribers.delete(sessionId);
    }
    return;
  }

  subscribers.forEach((subscriber) => subscriber(state));
}

export function applyLiveChatAction(
  sessionId: string,
  fallbackState: ChatState,
  action: ChatAction,
) {
  const nextState = chatReducer(liveChatStateCache.get(sessionId) ?? fallbackState, action);
  setLiveChatState(sessionId, nextState);
}

export function subscribeToLiveChatState(
  sessionId: string,
  subscriber: (state: ChatState | null) => void,
) {
  const existing =
    liveChatSubscribers.get(sessionId) ?? new Set<(state: ChatState | null) => void>();
  existing.add(subscriber);
  liveChatSubscribers.set(sessionId, existing);

  return () => {
    const current = liveChatSubscribers.get(sessionId);
    if (!current) return;
    current.delete(subscriber);
    if (current.size === 0) {
      liveChatSubscribers.delete(sessionId);
    }
  };
}
