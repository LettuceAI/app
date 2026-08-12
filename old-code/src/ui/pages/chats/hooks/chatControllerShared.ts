import type { Dispatch, MutableRefObject } from "react";

import { saveSession } from "../../../../core/storage/repo";
import type { Character, Scene, Session, StoredMessage } from "../../../../core/storage/schemas";
import type { ChatAction, ChatState } from "./chatReducer";

const sessionSaveQueue = new Map<string, Promise<void>>();

export interface LoggerLike {
  info: (...args: unknown[]) => void;
  error: (...args: unknown[]) => void;
}

export interface ChatControllerModuleContext {
  state: ChatState;
  dispatch: Dispatch<ChatAction>;
  messagesRef: MutableRefObject<StoredMessage[]>;
  abortedRequestIdsRef: MutableRefObject<Set<string>>;
  sessionOperationRef: MutableRefObject<boolean>;
  log: LoggerLike;
  persistSession: (session: Session) => Promise<void>;
  recordSessionTimestamp: (updatedAt: number) => void;
}

export interface ChatControllerPagingContext extends ChatControllerModuleContext {
  hasMoreMessagesBeforeRef: MutableRefObject<boolean>;
  loadingOlderRef: MutableRefObject<boolean>;
}

export async function queueSessionSave(session: Session): Promise<void> {
  const sessionId = session.id;
  const pendingSave = sessionSaveQueue.get(sessionId);
  if (pendingSave) {
    await pendingSave;
  }

  const savePromise = saveSession(session).finally(() => {
    if (sessionSaveQueue.get(sessionId) === savePromise) {
      sessionSaveQueue.delete(sessionId);
    }
  });

  sessionSaveQueue.set(sessionId, savePromise);
  return savePromise;
}

export function isStartingSceneMessage(message: StoredMessage): boolean {
  return message.role === "scene";
}

export function resolveSceneContent(scene: Scene): string {
  if (scene.selectedVariantId) {
    const selectedVariant = scene.variants?.find(
      (variant) => variant.id === scene.selectedVariantId,
    );
    if (selectedVariant?.content?.trim()) {
      return selectedVariant.content;
    }
  }

  if (scene.content?.trim()) {
    return scene.content;
  }

  return scene.direction?.trim() ?? "";
}

export function normalizeStartingSceneMessage(
  messages: StoredMessage[],
  character: Character,
  selectedSceneId?: string | null,
): StoredMessage[] {
  const sceneMessageIndex = messages.findIndex((message) => isStartingSceneMessage(message));
  if (sceneMessageIndex < 0) {
    return messages;
  }

  const selectedScene = character.scenes.find((scene) => scene.id === selectedSceneId);
  if (!selectedScene) {
    return messages;
  }

  const expectedSceneContent = resolveSceneContent(selectedScene).trim();
  if (!expectedSceneContent) {
    return messages;
  }

  const currentSceneMessage = messages[sceneMessageIndex];
  if (currentSceneMessage.sceneEdited) {
    return messages;
  }

  if (currentSceneMessage.content.trim() === expectedSceneContent) {
    return messages;
  }

  const nextMessages = [...messages];
  nextMessages[sceneMessageIndex] = {
    ...currentSceneMessage,
    content: expectedSceneContent,
  };
  return nextMessages;
}
