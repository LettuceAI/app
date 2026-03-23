import { invoke } from "@tauri-apps/api/core";
import type { StoredMessage, UsageSummary, ImageAttachment } from "../storage/schemas";
import { beginAsyncAction, endAsyncAction } from "../asyncTracker";

export interface ChatTurnResult {
  sessionId: string;
  sessionUpdatedAt: number;
  requestId?: string;
  userMessage: StoredMessage;
  assistantMessage: StoredMessage;
  usage?: UsageSummary;
}

export interface ChatRegenerateResult {
  sessionId: string;
  sessionUpdatedAt: number;
  requestId?: string;
  assistantMessage: StoredMessage;
}

export interface ChatContinueResult {
  sessionId: string;
  sessionUpdatedAt: number;
  requestId?: string;
  assistantMessage: StoredMessage;
}

export async function sendChatTurn(params: {
  sessionId: string;
  characterId: string;
  message: string;
  personaId?: string | null;
  swapPlaces?: boolean;
  stream?: boolean;
  requestId?: string;
  attachments?: ImageAttachment[];
}): Promise<ChatTurnResult> {
  const {
    sessionId,
    characterId,
    message,
    personaId,
    swapPlaces = false,
    stream = true,
    requestId,
    attachments = [],
  } = params;
  if (!message.trim() && attachments.length === 0) {
    throw new Error("Message cannot be empty");
  }

  if (requestId) beginAsyncAction(requestId, "chat_completion");
  try {
    return await invoke<ChatTurnResult>("chat_completion", {
      args: {
        sessionId,
        characterId,
        userMessage: message,
        personaId: personaId ?? null,
        swapPlaces,
        stream,
        requestId: requestId ?? null,
        attachments,
      },
    });
  } finally {
    if (requestId) endAsyncAction(requestId);
  }
}

export async function continueConversation(params: {
  sessionId: string;
  characterId: string;
  personaId?: string | null;
  swapPlaces?: boolean;
  stream?: boolean;
  requestId?: string;
}): Promise<ChatContinueResult> {
  const {
    sessionId,
    characterId,
    personaId,
    swapPlaces = false,
    stream = true,
    requestId,
  } = params;

  if (requestId) beginAsyncAction(requestId, "chat_continue");
  try {
    return await invoke<ChatContinueResult>("chat_continue", {
      args: {
        sessionId,
        characterId,
        personaId: personaId ?? null,
        swapPlaces,
        stream,
        requestId: requestId ?? null,
      },
    });
  } finally {
    if (requestId) endAsyncAction(requestId);
  }
}

export async function regenerateAssistantMessage(params: {
  sessionId: string;
  messageId: string;
  swapPlaces?: boolean;
  stream?: boolean;
  requestId?: string;
}): Promise<ChatRegenerateResult> {
  const { sessionId, messageId, swapPlaces = false, stream = true, requestId } = params;
  if (requestId) beginAsyncAction(requestId, "chat_regenerate");
  try {
    return await invoke<ChatRegenerateResult>("chat_regenerate", {
      args: {
        sessionId,
        messageId,
        swapPlaces,
        stream,
        requestId: requestId ?? null,
      },
    });
  } finally {
    if (requestId) endAsyncAction(requestId);
  }
}

export async function abortMessage(requestId: string): Promise<void> {
  endAsyncAction(requestId);
  return invoke<void>("abort_request", {
    requestId,
  });
}

export async function addChatMessageAttachment(params: {
  sessionId: string;
  characterId: string;
  messageId: string;
  role: "user" | "assistant";
  attachmentId: string;
  base64Data: string;
  mimeType: string;
  filename?: string;
  width?: number;
  height?: number;
}): Promise<StoredMessage> {
  return invoke<StoredMessage>("chat_add_message_attachment", {
    args: {
      sessionId: params.sessionId,
      characterId: params.characterId,
      messageId: params.messageId,
      role: params.role,
      attachmentId: params.attachmentId,
      base64Data: params.base64Data,
      mimeType: params.mimeType,
      filename: params.filename ?? null,
      width: params.width ?? null,
      height: params.height ?? null,
    },
  });
}

export async function generateSceneImageForMessage(params: {
  sessionId: string;
  messageId: string;
  attachmentId: string;
  scenePrompt: string;
}): Promise<StoredMessage> {
  return invoke<StoredMessage>("chat_generate_scene_image", {
    args: {
      sessionId: params.sessionId,
      messageId: params.messageId,
      attachmentId: params.attachmentId,
      scenePrompt: params.scenePrompt,
    },
  });
}

export async function generateScenePromptForMessage(params: {
  sessionId: string;
  messageId: string;
}): Promise<string> {
  return invoke<string>("chat_generate_scene_prompt", {
    args: {
      sessionId: params.sessionId,
      messageId: params.messageId,
    },
  });
}

export async function generateDesignReferenceDescription(params: {
  subjectName?: string | null;
  subjectDescription?: string | null;
  currentDescription?: string | null;
  avatarImage?: string | null;
  referenceImages?: string[];
  requestId?: string | null;
  stream?: boolean;
}): Promise<string> {
  const requestId = params.requestId ?? null;
  if (requestId) beginAsyncAction(requestId, "design_reference_writer");
  try {
    return await invoke<string>("chat_generate_design_reference_description", {
      args: {
        subjectName: params.subjectName ?? null,
        subjectDescription: params.subjectDescription ?? null,
        currentDescription: params.currentDescription ?? null,
        avatarImage: params.avatarImage ?? null,
        referenceImages: params.referenceImages ?? [],
        requestId,
        stream: params.stream ?? true,
      },
    });
  } finally {
    if (requestId) endAsyncAction(requestId);
  }
}
