import { useCallback, useEffect, useRef } from "react";
import { type as getPlatform } from "@tauri-apps/plugin-os";
import { impactFeedback } from "@tauri-apps/plugin-haptics";

import {
  addChatMessageAttachment,
  generateSceneImageForMessage,
  generateScenePromptForMessage,
} from "../../../../core/chat/manager";
import {
  generateImage,
  resolveGeneratedImageUrl,
  resolveImageGenerationOptions,
  resolveProviderCredential,
  type ImageGenerationRequest,
} from "../../../../core/image-generation";
import type { GeneratedImage } from "../../../../core/image-generation";
import { readSettings, SETTINGS_UPDATED_EVENT } from "../../../../core/storage/repo";
import type { ImageAttachment, Session, StoredMessage } from "../../../../core/storage/schemas";
import { isRenderableImageUrl } from "../../../../core/utils/image";
import { toast } from "../../../components/toast";
import type { ChatControllerModuleContext } from "./chatControllerShared";

const IMAGE_DIRECTIVE_RE = /<<image:(\{[\s\S]*?\})>>/g;

interface ImageGenConfig {
  modelName: string;
  providerId: string;
  credentialId: string;
}

type SceneGenerationMode = "auto" | "askFirst" | "manual";
export const SCENE_PROMPT_APPROVAL_EVENT = "lettuceai:scene-prompt-approval";

export interface ScenePromptApprovalDetail {
  sessionId: string;
  message: StoredMessage;
  scenePrompt: string;
}

interface UseChatEnhancementsControllerArgs {
  context: ChatControllerModuleContext;
}

export function useChatEnhancementsController({ context }: UseChatEnhancementsControllerArgs) {
  const { state, dispatch, messagesRef, abortedRequestIdsRef, persistSession } = context;
  const longPressTimerRef = useRef<number | null>(null);
  const processedImageDirectiveMessagesRef = useRef<Set<string>>(new Set());
  const imageGenConfigRef = useRef<ImageGenConfig | null>(null);
  const hapticsEnabledRef = useRef<boolean>(false);
  const hapticIntensityRef = useRef<any>("light");
  const sceneGenerationEnabledRef = useRef<boolean>(false);
  const sceneGenerationModeRef = useRef<SceneGenerationMode>("auto");
  const lastHapticTimeRef = useRef<number>(0);
  const platformRef = useRef<string>("");

  useEffect(() => {
    platformRef.current = getPlatform();

    const resetCachedConfigs = () => {
      imageGenConfigRef.current = null;
    };

    const updateHapticsState = async () => {
      resetCachedConfigs();
      try {
        const settings = await readSettings();
        const acc = settings.advancedSettings?.accessibility;
        hapticsEnabledRef.current = acc?.haptics ?? false;
        hapticIntensityRef.current = acc?.hapticIntensity ?? "light";
        sceneGenerationEnabledRef.current =
          settings.advancedSettings?.sceneGenerationEnabled ?? true;
        sceneGenerationModeRef.current =
          settings.advancedSettings?.sceneGenerationMode === "manual"
            ? "manual"
            : settings.advancedSettings?.sceneGenerationMode === "askFirst"
              ? "askFirst"
              : "auto";
      } catch {
        // ignore settings read failures
      }
    };

    void updateHapticsState();
    window.addEventListener(SETTINGS_UPDATED_EVENT, updateHapticsState);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, updateHapticsState);
  }, []);

  useEffect(() => {
    return () => {
      if (longPressTimerRef.current !== null) {
        window.clearTimeout(longPressTimerRef.current);
      }
    };
  }, []);

  const triggerTypingHaptic = useCallback(async () => {
    if (!hapticsEnabledRef.current) return;
    const isMobile = platformRef.current === "android" || platformRef.current === "ios";
    if (!isMobile) return;

    const now = Date.now();
    if (now - lastHapticTimeRef.current < 60) return;

    lastHapticTimeRef.current = now;
    try {
      await impactFeedback(hapticIntensityRef.current);
    } catch {
      // ignore haptics failures
    }
  }, []);

  const resolveDefaultImageGenConfig = useCallback(async (): Promise<ImageGenConfig | null> => {
    if (imageGenConfigRef.current) return imageGenConfigRef.current;

    const settings = await readSettings();
    const options = resolveImageGenerationOptions(settings);
    const firstModel = options.defaultModel;
    if (!firstModel) return null;

    const provider = resolveProviderCredential(
      options.providers,
      firstModel.providerId,
      firstModel.providerLabel,
    );
    if (!provider) return null;

    const config = {
      modelName: firstModel.name,
      providerId: firstModel.providerId,
      credentialId: provider.id,
    };
    imageGenConfigRef.current = config;
    return config;
  }, []);

  const dataUrlFromGeneratedImage = useCallback(
    async (generated: GeneratedImage): Promise<string> => {
      if (isRenderableImageUrl(generated.url)) {
        return generated.url!;
      }

      const src = await resolveGeneratedImageUrl(generated);
      if (!src) {
        throw new Error("Generated image has no asset or display url");
      }

      const response = await fetch(src);
      const blob = await response.blob();

      return await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onloadend = () => resolve(reader.result as string);
        reader.onerror = () => reject(new Error("Failed to read image blob"));
        reader.readAsDataURL(blob);
      });
    },
    [],
  );

  const imageInfoFromDataUrl = useCallback(async (dataUrl: string) => {
    const mimeMatch = dataUrl.match(/^data:([^;]+);base64,/);
    const mimeType = mimeMatch?.[1] ?? "image/png";

    const dimensions = await new Promise<{ width: number; height: number }>((resolve) => {
      const image = new Image();
      image.onload = () => resolve({ width: image.width, height: image.height });
      image.onerror = () => resolve({ width: 0, height: 0 });
      image.src = dataUrl;
    });

    return {
      mimeType,
      width: dimensions.width || undefined,
      height: dimensions.height || undefined,
    };
  }, []);

  const parseImageDirectives = useCallback((content: string) => {
    const directives: Array<{
      prompt: string;
      size?: string;
      n?: number;
      quality?: string;
      style?: string;
    }> = [];

    IMAGE_DIRECTIVE_RE.lastIndex = 0;
    const cleanContent = content.replace(IMAGE_DIRECTIVE_RE, (fullMatch, jsonStr) => {
      try {
        const parsed = JSON.parse(jsonStr);
        const prompt = typeof parsed?.prompt === "string" ? parsed.prompt.trim() : "";
        if (!prompt) return "";

        directives.push({
          prompt,
          size: typeof parsed?.size === "string" ? parsed.size : undefined,
          n: typeof parsed?.n === "number" ? parsed.n : undefined,
          quality: typeof parsed?.quality === "string" ? parsed.quality : undefined,
          style: typeof parsed?.style === "string" ? parsed.style : undefined,
        });
      } catch {
        return fullMatch;
      }

      return "";
    });

    return { cleanContent: cleanContent.trim(), directives };
  }, []);

  const parseSize = useCallback((size?: string) => {
    if (!size) return null;
    const match = size.match(/^(\d+)\s*x\s*(\d+)$/i);
    if (!match) return null;

    const width = Number(match[1]);
    const height = Number(match[2]);
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
      return null;
    }

    return { width, height };
  }, []);

  const getErrorMessage = useCallback((error: unknown) => {
    if (error instanceof Error) return error.message;
    if (typeof error === "string") return error;
    if (error && typeof error === "object" && "message" in error) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === "string" && message.trim()) return message;
    }
    return "Unknown error";
  }, []);

  const runInChatImageGeneration = useCallback(
    async (
      assistantMessageId: string,
      options?: { scenePrompt?: string | null; requestId?: string | null },
    ) => {
      if (!state.session || !state.character) return;
      let sceneGenerationEnabled = sceneGenerationEnabledRef.current;
      let sceneGenerationMode = sceneGenerationModeRef.current;
      try {
        const settings = await readSettings();
        sceneGenerationEnabled = settings.advancedSettings?.sceneGenerationEnabled ?? true;
        sceneGenerationMode =
          settings.advancedSettings?.sceneGenerationMode === "manual"
            ? "manual"
            : settings.advancedSettings?.sceneGenerationMode === "askFirst"
              ? "askFirst"
              : "auto";
        sceneGenerationEnabledRef.current = sceneGenerationEnabled;
        sceneGenerationModeRef.current = sceneGenerationMode;
      } catch {
        // Fall back to the last known value.
      }

      const currentSession = state.session;
      const currentCharacter = state.character;
      if (processedImageDirectiveMessagesRef.current.has(assistantMessageId)) return;
      const originatingRequestId = options?.requestId?.trim() ?? null;
      const requestWasAborted = () =>
        Boolean(originatingRequestId && abortedRequestIdsRef.current.has(originatingRequestId));

      const currentMessage = messagesRef.current.find(
        (message) => message.id === assistantMessageId,
      );
      if (!currentMessage) return;

      const { cleanContent, directives } = parseImageDirectives(currentMessage.content);
      const scenePrompt =
        sceneGenerationEnabled && sceneGenerationMode !== "manual"
          ? (options?.scenePrompt?.trim() ?? "")
          : "";
      const shouldAskForSceneApproval = Boolean(scenePrompt) && sceneGenerationMode === "askFirst";
      if (directives.length === 0 && !scenePrompt) return;

      const config = directives.length > 0 ? await resolveDefaultImageGenConfig() : null;
      const runnableDirectives = config ? directives : [];

      if (runnableDirectives.length === 0 && !scenePrompt) return;
      if (requestWasAborted()) return;

      processedImageDirectiveMessagesRef.current.add(assistantMessageId);

      const placeholderAttachments: ImageAttachment[] = [];
      for (const directive of runnableDirectives) {
        const count = Math.max(1, Math.min(4, directive.n ?? 1));
        const dimensions = parseSize(directive.size) ?? parseSize("1024x1024");
        for (let index = 0; index < count; index++) {
          placeholderAttachments.push({
            id: crypto.randomUUID(),
            data: "",
            mimeType: "image/webp",
            width: dimensions?.width,
            height: dimensions?.height,
          });
        }
      }
      const scenePlaceholder =
        scenePrompt && !shouldAskForSceneApproval
          ? {
              id: crypto.randomUUID(),
              data: "",
              mimeType: "image/webp",
              width: 1024,
              height: 1024,
            }
          : null;
      if (scenePlaceholder) {
        placeholderAttachments.push(scenePlaceholder);
      }

      const cleanupPlaceholderAttachments = async (placeholderIds: Iterable<string>) => {
        const ids = new Set(Array.from(placeholderIds));
        if (ids.size === 0) return;

        const latestMessage = messagesRef.current.find(
          (message) => message.id === assistantMessageId,
        );
        if (!latestMessage) return;

        const cleanedMessage: StoredMessage = {
          ...latestMessage,
          attachments: (latestMessage.attachments ?? []).filter(
            (attachment) => !ids.has(attachment.id),
          ),
        };

        const nextMessages = messagesRef.current.map((message) =>
          message.id === cleanedMessage.id ? cleanedMessage : message,
        );
        messagesRef.current = nextMessages;

        const nextSession: Session = {
          ...currentSession,
          messages: nextMessages,
          updatedAt: Date.now(),
        };

        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_MESSAGES", payload: nextMessages },
            { type: "SET_SESSION", payload: nextSession },
          ],
        });

        try {
          await persistSession(nextSession);
        } catch {
          // keep in-memory cleanup if persistence fails
        }
      };

      const updatedMessage: StoredMessage = {
        ...currentMessage,
        content: cleanContent,
        attachments: [...(currentMessage.attachments ?? []), ...placeholderAttachments],
      };

      const updatedMessages = messagesRef.current.map((message) =>
        message.id === assistantMessageId ? updatedMessage : message,
      );
      messagesRef.current = updatedMessages;

      const updatedSession: Session = {
        ...currentSession,
        messages: updatedMessages,
        updatedAt: Date.now(),
      };

      dispatch({
        type: "BATCH",
        actions: [
          { type: "SET_MESSAGES", payload: updatedMessages },
          { type: "SET_SESSION", payload: updatedSession },
        ],
      });

      try {
        await persistSession(updatedSession);
      } catch {
        // allow generation to continue even if placeholder persistence fails
      }

      if (requestWasAborted()) {
        await cleanupPlaceholderAttachments(
          placeholderAttachments.map((attachment) => attachment.id),
        );
        return;
      }

      for (
        let directiveIndex = 0, placeholderIndex = 0;
        directiveIndex < runnableDirectives.length;
        directiveIndex++
      ) {
        const directive = runnableDirectives[directiveIndex];
        const count = Math.max(1, Math.min(4, directive.n ?? 1));
        const placeholdersForDirective = placeholderAttachments.slice(
          placeholderIndex,
          placeholderIndex + count,
        );
        placeholderIndex += count;

        if (requestWasAborted()) {
          await cleanupPlaceholderAttachments(
            placeholderAttachments
              .slice(placeholderIndex - count)
              .map((attachment) => attachment.id),
          );
          return;
        }

        const request: ImageGenerationRequest = {
          prompt: directive.prompt,
          model: config!.modelName,
          providerId: config!.providerId,
          credentialId: config!.credentialId,
          size: directive.size ?? "1024x1024",
          n: count,
          quality: directive.quality,
          style: directive.style,
        };

        try {
          const response = await generateImage(request);
          if (requestWasAborted()) {
            await cleanupPlaceholderAttachments(
              placeholderAttachments
                .slice(placeholderIndex - count)
                .map((attachment) => attachment.id),
            );
            return;
          }
          const images = response.images.slice(0, placeholdersForDirective.length);

          for (let index = 0; index < images.length; index++) {
            const placeholderId = placeholdersForDirective[index]?.id;
            if (!placeholderId) continue;

            const dataUrl = await dataUrlFromGeneratedImage(images[index]);
            const info = await imageInfoFromDataUrl(dataUrl);

            const updated = await addChatMessageAttachment({
              sessionId: currentSession.id,
              characterId: currentCharacter.id,
              messageId: assistantMessageId,
              role: "assistant",
              attachmentId: placeholderId,
              base64Data: dataUrl,
              mimeType: info.mimeType,
              filename: directive.prompt,
              width: info.width,
              height: info.height,
            });

            const nextMessages = messagesRef.current.map((message) =>
              message.id === updated.id ? updated : message,
            );
            messagesRef.current = nextMessages;
            dispatch({ type: "SET_MESSAGES", payload: nextMessages });
          }
        } catch (error) {
          console.error("In-chat image generation failed:", error);
          const placeholderIds = new Set(
            placeholdersForDirective.map((placeholder) => placeholder.id),
          );
          const latestMessage = messagesRef.current.find(
            (message) => message.id === assistantMessageId,
          );
          if (!latestMessage || placeholderIds.size === 0) continue;

          const cleanedMessage: StoredMessage = {
            ...latestMessage,
            attachments: (latestMessage.attachments ?? []).filter(
              (attachment) => !placeholderIds.has(attachment.id),
            ),
          };
          const nextMessages = messagesRef.current.map((message) =>
            message.id === cleanedMessage.id ? cleanedMessage : message,
          );
          messagesRef.current = nextMessages;

          const nextSession: Session = {
            ...currentSession,
            messages: nextMessages,
            updatedAt: Date.now(),
          };

          dispatch({
            type: "BATCH",
            actions: [
              { type: "SET_MESSAGES", payload: nextMessages },
              { type: "SET_SESSION", payload: nextSession },
            ],
          });

          try {
            await persistSession(nextSession);
          } catch {
            // leave cleaned UI state in memory if persistence fails
          }
        }
      }

      if (shouldAskForSceneApproval) {
        if (requestWasAborted()) {
          return;
        }
        window.dispatchEvent(
          new CustomEvent<ScenePromptApprovalDetail>(SCENE_PROMPT_APPROVAL_EVENT, {
            detail: {
              sessionId: currentSession.id,
              message: updatedMessage,
              scenePrompt,
            },
          }),
        );
      }

      if (scenePrompt && scenePlaceholder) {
        if (requestWasAborted()) {
          await cleanupPlaceholderAttachments([scenePlaceholder.id]);
          return;
        }
        try {
          const updated = await generateSceneImageForMessage({
            sessionId: currentSession.id,
            messageId: assistantMessageId,
            attachmentId: scenePlaceholder.id,
            scenePrompt,
          });

          const nextMessages = messagesRef.current.map((message) =>
            message.id === updated.id ? updated : message,
          );
          messagesRef.current = nextMessages;
          dispatch({ type: "SET_MESSAGES", payload: nextMessages });
        } catch (error) {
          const latestMessage = messagesRef.current.find(
            (message) => message.id === assistantMessageId,
          );
          if (latestMessage) {
            const cleanedMessage: StoredMessage = {
              ...latestMessage,
              attachments: (latestMessage.attachments ?? []).filter(
                (attachment) => attachment.id !== scenePlaceholder.id,
              ),
            };
            const nextMessages = messagesRef.current.map((message) =>
              message.id === cleanedMessage.id ? cleanedMessage : message,
            );
            messagesRef.current = nextMessages;

            const nextSession: Session = {
              ...currentSession,
              messages: nextMessages,
              updatedAt: Date.now(),
            };

            dispatch({
              type: "BATCH",
              actions: [
                { type: "SET_MESSAGES", payload: nextMessages },
                { type: "SET_SESSION", payload: nextSession },
              ],
            });

            try {
              await persistSession(nextSession);
            } catch {
              // leave cleaned UI state in memory if persistence fails
            }
          }

          toast.error("Scene generation failed", getErrorMessage(error));
        }
      }
    },
    [
      abortedRequestIdsRef,
      dataUrlFromGeneratedImage,
      dispatch,
      getErrorMessage,
      imageInfoFromDataUrl,
      messagesRef,
      parseImageDirectives,
      parseSize,
      persistSession,
      resolveDefaultImageGenConfig,
      state.character,
      state.session,
    ],
  );

  const generateAiScenePrompt = useCallback(
    async (messageId: string): Promise<string> => {
      if (!state.session) {
        throw new Error("Session not loaded");
      }

      return generateScenePromptForMessage({
        sessionId: state.session.id,
        messageId,
      });
    },
    [state.session],
  );

  const applySceneImagePrompt = useCallback(
    async (message: StoredMessage, scenePrompt: string) => {
      if (!state.session) {
        throw new Error("Session not loaded");
      }

      const prompt = scenePrompt.trim();
      if (!prompt) {
        throw new Error("Scene prompt cannot be empty");
      }

      const attachmentId = crypto.randomUUID();
      const previousMessage = message;
      const placeholderAttachment: ImageAttachment = {
        id: attachmentId,
        data: "",
        mimeType: "image/webp",
        width: 1024,
        height: 1024,
        filename: prompt,
      };

      const optimisticMessage: StoredMessage = {
        ...previousMessage,
        attachments: [...(previousMessage.attachments ?? []), placeholderAttachment],
      };

      const optimisticMessages = messagesRef.current.map((entry) =>
        entry.id === optimisticMessage.id ? optimisticMessage : entry,
      );
      messagesRef.current = optimisticMessages;

      const optimisticSession: Session = {
        ...state.session,
        messages: optimisticMessages,
        updatedAt: Date.now(),
      };

      dispatch({
        type: "BATCH",
        actions: [
          { type: "SET_MESSAGES", payload: optimisticMessages },
          { type: "SET_SESSION", payload: optimisticSession },
        ],
      });

      try {
        const updated = await generateSceneImageForMessage({
          sessionId: state.session.id,
          messageId: previousMessage.id,
          attachmentId,
          scenePrompt: prompt,
        });

        const nextMessages = messagesRef.current.map((entry) =>
          entry.id === updated.id ? updated : entry,
        );
        messagesRef.current = nextMessages;

        const nextSession: Session = {
          ...state.session,
          messages: nextMessages,
          updatedAt: Date.now(),
        };

        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_MESSAGES", payload: nextMessages },
            { type: "SET_SESSION", payload: nextSession },
          ],
        });
      } catch (error) {
        const revertedMessages = messagesRef.current.map((entry) =>
          entry.id === previousMessage.id ? previousMessage : entry,
        );
        messagesRef.current = revertedMessages;

        const revertedSession: Session = {
          ...state.session,
          messages: revertedMessages,
          updatedAt: Date.now(),
        };

        dispatch({
          type: "BATCH",
          actions: [
            { type: "SET_MESSAGES", payload: revertedMessages },
            { type: "SET_SESSION", payload: revertedSession },
          ],
        });

        throw error;
      }
    },
    [dispatch, messagesRef, state.session],
  );

  const initializeLongPressTimer = useCallback((timer: number | null) => {
    if (timer === null) {
      if (longPressTimerRef.current !== null) {
        window.clearTimeout(longPressTimerRef.current);
        longPressTimerRef.current = null;
      }
      return;
    }

    longPressTimerRef.current = timer;
  }, []);

  return {
    applySceneImagePrompt,
    generateAiScenePrompt,
    initializeLongPressTimer,
    runInChatImageGeneration,
    triggerTypingHaptic,
  };
}
