import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import {
  Sparkles,
  Loader2,
  Check,
  PenLine,
  RefreshCw,
  Image as ImageIcon,
  Eye,
  User,
  BookOpen,
} from "lucide-react";
import { TopNav } from "../../components/App";
import { cn, typography, animations, radius } from "../../design-tokens";
import { BottomMenu, MenuButton, MenuSection } from "../../components";
import { MarkdownRenderer } from "../chats/components/MarkdownRenderer";
import { CharacterPreviewCard, PersonaPreviewCard, LorebookPreviewCard } from "./components";
import { CreationHelperFooter } from "./components/CreationHelperFooter";
import { ReferenceSelector, ReferenceAvatar, Reference } from "./components/ReferenceSelector";
import { listCharacters, listPersonas, readSettings } from "../../../core/storage/repo";

interface CreationMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  toolCalls: ToolCall[];
  toolResults: ToolResult[];
  createdAt: number;
}

interface ToolCall {
  id: string;
  name: string;
  arguments: unknown;
  rawArguments?: string | null;
}

interface ToolResult {
  toolCallId: string;
  result: unknown;
  success: boolean;
}

interface DraftCharacter {
  name: string | null;
  definition?: string | null;
  description: string | null;
  scenes: DraftScene[];
  defaultSceneId: string | null;
  avatarPath: string | null;
  backgroundImagePath: string | null;
  disableAvatarGradient: boolean;
  defaultModelId: string | null;
  promptTemplateId: string | null;
}

interface DraftScene {
  id: string;
  content: string;
  direction: string | null;
}

interface PreviewPersona {
  id?: string;
  title?: string;
  description?: string;
  avatarPath?: string | null;
  isDefault?: boolean;
}

interface PreviewLorebook {
  id?: string;
  name?: string;
}

interface PreviewLorebookEntry {
  id: string;
  lorebookId?: string;
  title?: string;
  content?: string;
  keywords?: string[];
  alwaysActive?: boolean;
  enabled?: boolean;
  displayOrder?: number;
}

interface CreationSession {
  id: string;
  messages: CreationMessage[];
  draft: DraftCharacter;
  draftHistory: DraftCharacter[];
  creationGoal: "character" | "persona" | "lorebook";
  creationMode: "create" | "edit";
  targetType?: "character" | "persona" | "lorebook" | null;
  targetId?: string | null;
  status: "active" | "previewShown" | "completed" | "cancelled";
  createdAt: number;
  updatedAt: number;
}

interface CreationSessionSummary {
  id: string;
  creationMode: "create" | "edit";
  status: "active" | "previewShown" | "completed" | "cancelled";
}

interface ImageAttachment {
  id: string;
  data: string;
  mimeType: string;
  filename?: string;
}

interface UploadedImage {
  id: string;
  data: string;
  mimeType: string;
}

interface ImageGenerationEntry {
  id: string;
  toolCallId: string;
  status: "pending" | "done" | "error";
  imageId?: string;
  createdAt: number;
}

// Component to fetch and display uploaded image thumbnail
function ImageThumbnail({
  sessionId,
  imageId,
  filename,
  localCache,
}: {
  sessionId: string;
  imageId: string;
  filename: string;
  localCache?: Record<string, string>;
}) {
  const [imageUrl, setImageUrl] = useState<string | null>(localCache?.[imageId] || null);
  const [loading, setLoading] = useState(!localCache?.[imageId]);

  useEffect(() => {
    if (localCache?.[imageId]) {
      setImageUrl(localCache[imageId]);
      setLoading(false);
      return;
    }

    let active = true;
    const fetchImage = async () => {
      try {
        const img = await invoke<UploadedImage | null>("creation_helper_get_uploaded_image", {
          sessionId,
          imageId,
        });
        if (active && img) {
          setImageUrl(img.data);
        }
      } catch (err) {
        console.error("Failed to load image thumbnail:", err);
      } finally {
        if (active) setLoading(false);
      }
    };
    fetchImage();
    return () => {
      active = false;
    };
  }, [sessionId, imageId, localCache?.[imageId]]);

  if (loading) return <div className="h-20 w-20 animate-pulse bg-fg/10 rounded-md" />;
  if (!imageUrl)
    return (
      <div className="flex items-center gap-1.5 px-2 py-1 rounded-full text-xs bg-danger/20 text-danger">
        <ImageIcon className="h-3 w-3" />
        <span>Failed to load</span>
      </div>
    );

  return (
    <div className="group relative h-24 w-24 overflow-hidden rounded-lg border border-fg/10 bg-surface-el/20">
      <img
        src={imageUrl.startsWith("data:") ? imageUrl : `data:image/png;base64,${imageUrl}`}
        alt={filename}
        className="h-full w-full object-cover transition-transform group-hover:scale-105"
      />
      <div className="absolute inset-0 bg-linear-to-t from-black/60 to-transparent opacity-0 group-hover:opacity-100 transition-opacity flex items-end p-2">
        <span className="text-[10px] text-fg truncate w-full">{filename}</span>
      </div>
    </div>
  );
}

function GeneratedImagePreview({
  sessionId,
  imageId,
  label,
  size = "md",
  className,
}: {
  sessionId: string;
  imageId: string;
  label: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}) {
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const sizeClass = size === "lg" ? "h-72 w-72" : size === "sm" ? "h-36 w-36" : "h-40 w-40";

  useEffect(() => {
    let active = true;
    const fetchImage = async () => {
      try {
        const img = await invoke<UploadedImage | null>("creation_helper_get_uploaded_image", {
          sessionId,
          imageId,
        });
        if (active && img) {
          setImageUrl(img.data);
        }
      } catch (err) {
        console.error("Failed to load generated image:", err);
      } finally {
        if (active) setLoading(false);
      }
    };
    fetchImage();
    return () => {
      active = false;
    };
  }, [sessionId, imageId]);

  return (
    <div className={cn("flex flex-col items-center gap-2", className)}>
      <div
        className={cn(
          sizeClass,
          "overflow-hidden rounded-2xl border border-fg/10 bg-fg/5 flex items-center justify-center",
        )}
      >
        {loading ? (
          <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
        ) : imageUrl ? (
          <img
            src={imageUrl.startsWith("data:") ? imageUrl : `data:image/png;base64,${imageUrl}`}
            alt={label}
            className="h-full w-full object-cover"
          />
        ) : (
          <span className="text-xs text-fg/40">Failed to load</span>
        )}
      </div>
    </div>
  );
}

export function CreationHelperPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const creationGoalParam = searchParams.get("goal");
  const sessionIdParam = searchParams.get("sessionId");
  const modeParam = searchParams.get("mode");
  const targetTypeParam = searchParams.get("targetType");
  const targetIdParam = searchParams.get("targetId");
  const creationGoal: CreationSession["creationGoal"] =
    creationGoalParam === "persona" || creationGoalParam === "lorebook"
      ? creationGoalParam
      : "character";
  const creationMode: CreationSession["creationMode"] = modeParam === "edit" ? "edit" : "create";
  const targetType: CreationSession["targetType"] =
    targetTypeParam === "character" ||
    targetTypeParam === "persona" ||
    targetTypeParam === "lorebook"
      ? targetTypeParam
      : null;
  const targetId = targetIdParam?.trim() || null;
  const [session, setSession] = useState<CreationSession | null>(null);
  const [smartToolSelection, setSmartToolSelection] = useState(true);
  const [inputValue, setInputValue] = useState("");
  const [sending, setSending] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [showConfirmation, setShowConfirmation] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingAttachments, setPendingAttachments] = useState<ImageAttachment[]>([]);
  const [references, setReferences] = useState<Reference[]>([]);
  const [messageReferences, setMessageReferences] = useState<Record<string, Reference[]>>({});
  const [messageDisplayContent, setMessageDisplayContent] = useState<Record<string, string>>({});
  const [showReferenceSelector, setShowReferenceSelector] = useState(false);
  const [referenceSelectorType, setReferenceSelectorType] = useState<"character" | "persona">(
    "character",
  );
  // Entity avatar lookup: maps entityId -> avatarPath
  const [entityAvatars, setEntityAvatars] = useState<Record<string, string>>({});
  const [localImageCache, setLocalImageCache] = useState<Record<string, string>>({});
  const [selectedTool, setSelectedTool] = useState<{
    call: ToolCall;
    result: ToolResult;
  } | null>(null);
  const [showToolDetail, setShowToolDetail] = useState(false);
  const [imagePreview, setImagePreview] = useState<{ id: string; label: string } | null>(null);
  const [streamingContent, setStreamingContent] = useState<string>("");
  const [, setStreamingReasoning] = useState<string>("");
  const [activeTools, setActiveTools] = useState<ToolCall[]>([]);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamUnlistenRef = useRef<(() => void) | null>(null);
  const streamingContentRef = useRef<string>("");
  const lastActionRef = useRef<"send" | "regenerate" | null>(null);
  const lastSendSnapshotRef = useRef<{
    draft: string;
    references: Reference[];
    attachments: ImageAttachment[];
  } | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const initGuardRef = useRef<string | null>(null);
  const sendingRef = useRef(false);
  const [imageGenerations, setImageGenerations] = useState<ImageGenerationEntry[]>([]);

  const resolveErrorMessage = useCallback((err: unknown, fallback: string) => {
    if (typeof err === "string") return err;
    if (!err || typeof err !== "object") return fallback;
    const anyErr = err as any;
    if (anyErr.message) return String(anyErr.message);
    if (anyErr.error) {
      if (typeof anyErr.error === "string") return anyErr.error;
      if (anyErr.error?.message) return String(anyErr.error.message);
      try {
        return JSON.stringify(anyErr.error);
      } catch {
        return fallback;
      }
    }
    if (anyErr.data) {
      if (typeof anyErr.data === "string") return anyErr.data;
      if (anyErr.data?.message) return String(anyErr.data.message);
      if (anyErr.data?.error) {
        if (typeof anyErr.data.error === "string") return anyErr.data.error;
        if (anyErr.data.error?.message) return String(anyErr.data.error.message);
      }
    }
    return fallback;
  }, []);
  const activeGoal = session?.creationGoal ?? creationGoal;
  const goalLabel = smartToolSelection
    ? activeGoal === "persona"
      ? "Persona"
      : activeGoal === "lorebook"
        ? "Lorebook"
        : "Character"
    : "Creator";

  // Load entity avatars for reference lookup
  useEffect(() => {
    const loadEntityAvatars = async () => {
      try {
        const [characters, personas] = await Promise.all([listCharacters(), listPersonas()]);
        const lookup: Record<string, string> = {};
        characters.forEach((c) => {
          if (c.avatarPath) lookup[c.id] = c.avatarPath;
        });
        personas.forEach((p) => {
          if (p.avatarPath) lookup[p.id] = p.avatarPath;
        });
        setEntityAvatars(lookup);
      } catch (err) {
        console.error("Failed to load entity avatars:", err);
      }
    };
    loadEntityAvatars();
  }, []);

  useEffect(() => {
    sessionIdRef.current = session?.id ?? null;
  }, [session?.id]);

  useEffect(() => {
    sendingRef.current = sending;
  }, [sending]);

  // Initialize session
  useEffect(() => {
    const initSession = async () => {
      const initKey = `${creationGoal}:${sessionIdParam ?? ""}:${creationMode}:${targetType ?? ""}:${targetId ?? ""}`;
      if (initGuardRef.current === initKey) {
        return;
      }
      initGuardRef.current = initKey;
      try {
        const settings = await readSettings();
        const smartSelection = settings.advancedSettings?.creationHelperSmartToolSelection ?? true;
        setSmartToolSelection(smartSelection);

        let resumedSession: CreationSession | null = null;
        const requestedSessionId = sessionIdParam?.trim() || null;

        if (requestedSessionId) {
          resumedSession = await invoke<CreationSession | null>("creation_helper_get_session", {
            sessionId: requestedSessionId,
          });
        }

        if (!resumedSession && creationMode !== "edit") {
          const summaries = await invoke<CreationSessionSummary[]>(
            "creation_helper_list_sessions",
            {
              creationGoal,
            },
          );
          const latestCreate = summaries.find(
            (s) => s.creationMode === "create" && s.status !== "completed",
          );
          if (latestCreate) {
            resumedSession = await invoke<CreationSession | null>("creation_helper_get_session", {
              sessionId: latestCreate.id,
            });
          }
        }

        if (resumedSession) {
          setSession(resumedSession);
          return;
        }

        const newSession = await invoke<CreationSession>("creation_helper_start", {
          creationGoal,
          creationMode,
          targetType,
          targetId,
        });
        setSession(newSession);

        // Send initial greeting
        const greetingSession = await invoke<CreationSession>("creation_helper_send_message", {
          sessionId: newSession.id,
          message:
            creationMode === "edit"
              ? smartSelection
                ? `Hi! I want to edit this ${targetType ?? creationGoal} (id: ${targetId ?? "unknown"}).`
                : "Hi! I want to edit an existing item."
              : smartSelection
                ? creationGoal === "persona"
                  ? "Hi! I want to create a new persona."
                  : creationGoal === "lorebook"
                    ? "Hi! I want to create a new lorebook."
                    : "Hi! I want to create a new character."
                : "Hi! I want to create something new.",
          uploadedImages: null,
        });
        setSession(greetingSession);
      } catch (err) {
        console.error("Failed to start creation helper:", err);
        setError("Failed to start the creation helper. Please try again.");
      }
    };

    initSession();
  }, [creationGoal, sessionIdParam, creationMode, targetType, targetId]);

  useEffect(() => {
    setSession(null);
    setInputValue("");
    setSending(false);
    setShowPreview(false);
    setShowConfirmation(false);
    setError(null);
    setPendingAttachments([]);
    setReferences([]);
    setMessageReferences({});
    setMessageDisplayContent({});
    setSelectedTool(null);
    setShowToolDetail(false);
    setStreamingContent("");
    streamingContentRef.current = "";
    setStreamingReasoning("");
    setActiveTools([]);
    lastActionRef.current = null;
    lastSendSnapshotRef.current = null;
    setImageGenerations([]);
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current();
      streamUnlistenRef.current = null;
    }
  }, [creationGoal, sessionIdParam, creationMode, targetType, targetId]);

  useEffect(() => {
    if (activeTools.length === 0) return;
    const generationCalls = activeTools.filter((call) => call.name === "generate_image");
    if (generationCalls.length === 0) return;
    setImageGenerations((prev) => {
      const existingIds = new Set(prev.map((entry) => entry.toolCallId));
      const next = [...prev];
      for (const call of generationCalls) {
        if (!existingIds.has(call.id)) {
          next.push({
            id: `__image_generation__${call.id}`,
            toolCallId: call.id,
            status: "pending",
            createdAt: Date.now(),
          });
        }
      }
      return next;
    });
  }, [activeTools]);

  useEffect(() => {
    if (!session?.messages?.length) return;
    setImageGenerations((prev) => {
      const next = [...prev];
      let changed = false;
      for (const message of session.messages) {
        if (!message.toolResults?.length || !message.toolCalls?.length) continue;
        for (const result of message.toolResults) {
          const toolCall = message.toolCalls.find((call) => call.id === result.toolCallId);
          if (!toolCall || toolCall.name !== "generate_image") continue;
          const entryIndex = next.findIndex((entry) => entry.toolCallId === result.toolCallId);
          const resObj = result.result as { image_id?: string; imageId?: string };
          const imageId = resObj?.image_id ?? resObj?.imageId;
          if (entryIndex >= 0) {
            next[entryIndex] = {
              ...next[entryIndex],
              status: result.success ? "done" : "error",
              imageId,
            };
          } else {
            next.push({
              id: `__image_generation__${result.toolCallId}`,
              toolCallId: result.toolCallId,
              status: result.success ? "done" : "error",
              imageId,
              createdAt: message.createdAt ?? Date.now(),
            });
          }
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [session?.messages]);

  // Listen for updates from backend
  useEffect(() => {
    const unlisten = listen("creation-helper-update", (event) => {
      const payload = event.payload as {
        sessionId: string;
        draft: DraftCharacter;
        status: string;
        messages: CreationMessage[];
      };

      if (session && payload.sessionId === session.id) {
        setSession((prev) =>
          prev
            ? {
                ...prev,
                draft: payload.draft,
                status: payload.status as CreationSession["status"],
                messages: payload.messages,
              }
            : null,
        );

        // Only UI state like preview:
        if (payload.status === "previewShown") {
          setShowPreview(true);
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [session?.id]);

  useEffect(() => {
    return () => {
      if (streamUnlistenRef.current) {
        streamUnlistenRef.current();
        streamUnlistenRef.current = null;
      }
      const sessionId = sessionIdRef.current;
      if (sessionId && sendingRef.current) {
        invoke("creation_helper_cancel", { sessionId }).catch(console.error);
      }
    };
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session?.messages, streamingContent]);

  const handleSend = useCallback(async () => {
    if (!session || !inputValue.trim() || sending) return;

    // Build message with references
    let message = inputValue.trim();

    if (references.length > 0) {
      const referenceText = references
        .map((ref) => {
          // Include ID in format: [Referenced Character: "Name" (id:abc-123)]
          if (ref.type === "character") {
            return `[Referenced Character: "${ref.name}" (id:${ref.id})]\n${ref.description || "No definition available."}`;
          } else {
            return `[Referenced Persona: "${ref.name}" (id:${ref.id})]\n${ref.description || "No description available."}`;
          }
        })
        .join("\n\n");

      message = `${message}\n\n---\n${referenceText}`;
    }

    // Append pending attachments info
    if (pendingAttachments.length > 0) {
      const attachmentText = pendingAttachments
        .map((att) => `[Uploaded Image: "${att.filename || "image.png"}" (id:${att.id})]`)
        .join("\n");

      message = `${message}\n\n---\n${attachmentText}`;
    }

    const optimisticId = crypto.randomUUID();
    const displayContent = inputValue.trim();

    // Store references for this message
    if (references.length > 0) {
      setMessageReferences((prev) => ({
        ...prev,
        [optimisticId]: [...references],
      }));
    }

    // Store the display content (without reference text) for this message
    setMessageDisplayContent((prev) => ({
      ...prev,
      [optimisticId]: displayContent,
    }));

    // Optimistic UI update (show original message without reference details)
    const userMsg: CreationMessage = {
      id: optimisticId,
      role: "user",
      content: message,
      toolCalls: [],
      toolResults: [],
      createdAt: Date.now(),
    };

    setSession((prev) =>
      prev
        ? {
            ...prev,
            messages: [...prev.messages, userMsg],
          }
        : null,
    );

    setSending(true);
    setError(null);
    setStreamingContent("");
    streamingContentRef.current = "";
    setStreamingReasoning("");
    setActiveTools([]);
    lastActionRef.current = "send";

    const requestId = crypto.randomUUID();
    let unlistenStream: (() => void) | null = null;
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current();
      streamUnlistenRef.current = null;
    }

    // Prepare images for upload
    const imagesToUpload =
      pendingAttachments.length > 0
        ? pendingAttachments.map((att) => ({
            id: att.id,
            data: att.data,
            mimeType: att.mimeType,
          }))
        : null;

    lastSendSnapshotRef.current = {
      draft: inputValue,
      references: [...references],
      attachments: [...pendingAttachments],
    };

    // Clear inputs immediately for better UX
    setInputValue("");
    setReferences([]);
    setPendingAttachments([]);

    try {
      unlistenStream = await listen<any>(`api-normalized://${requestId}`, (event) => {
        let payload: { type?: string; data?: any } | null = null;
        try {
          payload =
            typeof event.payload === "string"
              ? JSON.parse(event.payload)
              : (event.payload as { type?: string; data?: any });
        } catch (err) {
          console.error("Failed to parse streaming payload:", err);
          return;
        }

        if (!payload?.type) return;

        if (payload.type === "delta" && payload.data?.text) {
          streamingContentRef.current += payload.data.text;
          setStreamingContent(streamingContentRef.current);
        } else if (payload.type === "reasoning" || payload.type === "thought") {
          if (payload.data?.text) {
            setStreamingReasoning((prev) => prev + payload.data.text);
          }
        } else if (payload.type === "toolCall" || payload.type === "tool_call") {
          const calls = Array.isArray(payload.data) ? payload.data : payload.data?.calls;
          if (calls?.length) {
            setActiveTools((prev) => {
              const merged = new Map(prev.map((call) => [call.id, call]));
              for (const call of calls) {
                merged.set(call.id, call);
              }
              return Array.from(merged.values());
            });
          }
        } else if (payload.type === "error") {
          const message = resolveErrorMessage(
            payload.data ?? payload,
            "Streaming error. Please try again.",
          );
          setError(message);
          setStreamingContent("");
          streamingContentRef.current = "";
          setStreamingReasoning("");
          setActiveTools([]);
          const snapshot = lastSendSnapshotRef.current;
          if (snapshot) {
            setInputValue((prev) => (prev.trim() ? prev : snapshot.draft));
            setReferences((prev) => (prev.length ? prev : snapshot.references));
            setPendingAttachments((prev) => (prev.length ? prev : snapshot.attachments));
          }
        }
      });
      streamUnlistenRef.current = unlistenStream;

      const updatedSession = await invoke<CreationSession>("creation_helper_send_message", {
        sessionId: session.id,
        message,
        uploadedImages: imagesToUpload,
        requestId,
      });

      setSession(updatedSession);

      // Check for tool actions that trigger UI
      const lastMessage = updatedSession.messages[updatedSession.messages.length - 1];
      if (!streamingContentRef.current.trim()) {
        if (!lastMessage || lastMessage.role !== "assistant" || !lastMessage.content?.trim()) {
          setError("Smart Creator failed to generate a response.");
          const snapshot = lastSendSnapshotRef.current;
          if (snapshot) {
            setInputValue((prev) => (prev.trim() ? prev : snapshot.draft));
            setReferences((prev) => (prev.length ? prev : snapshot.references));
            setPendingAttachments((prev) => (prev.length ? prev : snapshot.attachments));
          }
        }
      }
      if (lastMessage?.toolResults) {
        for (const result of lastMessage.toolResults) {
          const resObj = result.result as any;
          if (resObj && typeof resObj === "object") {
            const action = resObj.action;
            if (action === "show_preview" || action === "request_confirmation") {
              setShowPreview(true);
              if (action === "request_confirmation") {
                setShowConfirmation(true);
              }
            }
          }
        }
      }
    } catch (err: any) {
      console.error("Failed to send message:", err);
      setError(resolveErrorMessage(err, "Failed to send message. Please try again."));

      // Remove optimistic message on failure
      setSession((prev) =>
        prev
          ? {
              ...prev,
              messages: prev.messages.filter((m) => m.id !== optimisticId),
            }
          : null,
      );

      // Restore inputs so user can retry
      setInputValue(inputValue);
      setReferences(references);
      setPendingAttachments(pendingAttachments);
    } finally {
      if (unlistenStream) {
        unlistenStream();
      }
      if (streamUnlistenRef.current === unlistenStream) {
        streamUnlistenRef.current = null;
      }
      setSending(false);
      setStreamingContent("");
      streamingContentRef.current = "";
      setStreamingReasoning("");
      setActiveTools([]);
    }
  }, [session, inputValue, sending, references, pendingAttachments]);

  const handleRegenerate = useCallback(async () => {
    if (!session || sending) return;

    // Find last assistant message
    const assistantMessages = session.messages.filter((m) => m.role === "assistant");
    const lastAssistantMsg = assistantMessages[assistantMessages.length - 1];

    if (!lastAssistantMsg) return;

    // Optimistic Update: Remove message and revert draft state
    setSession((prev) => {
      if (!prev) return null;
      const nextDraftHistory = [...prev.draftHistory];
      const revertedDraft = nextDraftHistory.pop() || prev.draft;

      return {
        ...prev,
        messages: prev.messages.filter((m) => m.id !== lastAssistantMsg.id),
        draft: revertedDraft,
        draftHistory: nextDraftHistory,
      };
    });

    setSending(true);
    setError(null);
    setStreamingContent("");
    streamingContentRef.current = "";
    setStreamingReasoning("");
    setActiveTools([]);
    lastActionRef.current = "regenerate";

    const requestId = crypto.randomUUID();
    let unlistenStream: (() => void) | null = null;
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current();
      streamUnlistenRef.current = null;
    }

    try {
      unlistenStream = await listen<any>(`api-normalized://${requestId}`, (event) => {
        let payload: { type?: string; data?: any } | null = null;
        try {
          payload =
            typeof event.payload === "string"
              ? JSON.parse(event.payload)
              : (event.payload as { type?: string; data?: any });
        } catch (err) {
          console.error("Failed to parse streaming payload:", err);
          return;
        }

        if (!payload?.type) return;

        if (payload.type === "delta" && payload.data?.text) {
          streamingContentRef.current += payload.data.text;
          setStreamingContent(streamingContentRef.current);
        } else if (payload.type === "reasoning" || payload.type === "thought") {
          if (payload.data?.text) {
            setStreamingReasoning((prev) => prev + payload.data.text);
          }
        } else if (payload.type === "toolCall" || payload.type === "tool_call") {
          const calls = Array.isArray(payload.data) ? payload.data : payload.data?.calls;
          if (calls?.length) {
            setActiveTools((prev) => {
              const merged = new Map(prev.map((call) => [call.id, call]));
              for (const call of calls) {
                merged.set(call.id, call);
              }
              return Array.from(merged.values());
            });
          }
        } else if (payload.type === "error") {
          const message = resolveErrorMessage(
            payload.data ?? payload,
            "Streaming error. Please try again.",
          );
          setError(message);
          setStreamingContent("");
          streamingContentRef.current = "";
          setStreamingReasoning("");
          setActiveTools([]);
        }
      });
      streamUnlistenRef.current = unlistenStream;

      const updatedSession = await invoke<CreationSession>("creation_helper_regenerate", {
        sessionId: session.id,
        requestId,
      });

      setSession(updatedSession);

      // Check for tool actions that trigger UI
      const lastMessage = updatedSession.messages[updatedSession.messages.length - 1];
      if (!streamingContentRef.current.trim()) {
        if (!lastMessage || lastMessage.role !== "assistant" || !lastMessage.content?.trim()) {
          setError("Smart Creator failed to generate a response.");
        }
      }
      if (lastMessage?.toolResults) {
        for (const result of lastMessage.toolResults) {
          const resObj = result.result as any;
          if (resObj && typeof resObj === "object") {
            const action = resObj.action;
            if (action === "show_preview" || action === "request_confirmation") {
              setShowPreview(true);
              if (action === "request_confirmation") {
                setShowConfirmation(true);
              }
            }
          }
        }
      }
    } catch (err: any) {
      console.error("Failed to regenerate:", err);
      setError(resolveErrorMessage(err, "Failed to regenerate. Please try again."));
    } finally {
      if (unlistenStream) {
        unlistenStream();
      }
      if (streamUnlistenRef.current === unlistenStream) {
        streamUnlistenRef.current = null;
      }
      setSending(false);
      setStreamingContent("");
      streamingContentRef.current = "";
      setStreamingReasoning("");
      setActiveTools([]);
    }
  }, [session, sending]);

  const handleUseCharacter = useCallback(async () => {
    if (!session) return;

    try {
      const draft = await invoke<DraftCharacter>("creation_helper_complete", {
        sessionId: session.id,
      });

      if (
        session.creationMode === "edit" &&
        session.targetType === "character" &&
        session.targetId
      ) {
        navigate(`/settings/characters/${session.targetId}/edit`);
      } else {
        // Navigate to create character page with pre-filled data
        navigate("/create/character", {
          state: { draftCharacter: draft },
        });
      }
    } catch (err: any) {
      console.error("Failed to complete character:", err);
      setError(resolveErrorMessage(err, "Failed to save character."));
    }
  }, [session, navigate]);

  const handleEditManually = useCallback(() => {
    if (!session?.draft) return;
    navigate("/create/character", {
      state: { draftCharacter: session.draft },
    });
  }, [session, navigate]);

  const handleAbort = useCallback(() => {
    if (!session) return;
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current();
      streamUnlistenRef.current = null;
    }
    invoke("creation_helper_cancel", { sessionId: session.id })
      .then(() => {
        setSending(false);
        setStreamingContent("");
        streamingContentRef.current = "";
        setStreamingReasoning("");
        setActiveTools([]);
        setError("Generation cancelled.");
      })
      .catch(console.error);
  }, [session]);

  const handleBack = () => {
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current();
      streamUnlistenRef.current = null;
    }
    if (session && sending) {
      invoke("creation_helper_cancel", { sessionId: session.id }).catch(console.error);
    }
    setSending(false);
    setStreamingContent("");
    streamingContentRef.current = "";
    setStreamingReasoning("");
    setActiveTools([]);
    navigate(-1);
  };

  // Tool display helpers
  const getToolDisplayName = (toolName: string): string => {
    const names: Record<string, string> = {
      set_character_name: "Set name",
      set_character_definition: "Set definition",
      set_character_description: "Set definition",
      add_scene: "Add scene",
      update_scene: "Update scene",
      toggle_avatar_gradient: "Toggle gradient",
      set_default_model: "Set model",
      set_system_prompt: "Set prompt",
      use_uploaded_image_as_avatar: "Set avatar",
      use_uploaded_image_as_chat_background: "Set background",
      show_preview: "Show preview",
      request_confirmation: "Ready to save",
      generate_avatar: "Generate avatar",
      list_personas: "List personas",
      upsert_persona: "Save persona",
      use_uploaded_image_as_persona_avatar: "Set persona avatar",
      delete_persona: "Delete persona",
      get_default_persona: "Get default persona",
      list_lorebooks: "List lorebooks",
      upsert_lorebook: "Save lorebook",
      delete_lorebook: "Delete lorebook",
      list_lorebook_entries: "List lorebook entries",
      get_lorebook_entry: "Get lorebook entry",
      upsert_lorebook_entry: "Save lorebook entry",
      delete_lorebook_entry: "Delete lorebook entry",
      create_blank_lorebook_entry: "Create lorebook entry",
      reorder_lorebook_entries: "Reorder lorebook entries",
      list_character_lorebooks: "List character lorebooks",
      set_character_lorebooks: "Set character lorebooks",
      generate_image: "Generate image",
    };
    return names[toolName] || toolName;
  };

  // Thinking indicator component
  const TypingIndicator = () => (
    <motion.div
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex items-center gap-2"
      aria-label="Assistant is typing"
      aria-live="polite"
    >
      <div className="flex items-center gap-1">
        <span className="typing-dot" />
        <span className="typing-dot" style={{ animationDelay: "0.2s" }} />
        <span className="typing-dot" style={{ animationDelay: "0.4s" }} />
      </div>
    </motion.div>
  );

  const displayMessages = (() => {
    if (!session) return [];

    const msgs = [...session.messages];

    if (sending) {
      msgs.push({
        id: "__streaming__",
        role: "assistant",
        content: streamingContent || "",
        toolCalls: activeTools,
        toolResults: [],
        createdAt: Date.now(),
      });
    }

    if (imageGenerations.length > 0) {
      const imageMessages = imageGenerations.map((entry) => ({
        id: entry.id,
        role: "system" as const,
        content: "",
        toolCalls: [],
        toolResults: [],
        createdAt: entry.createdAt,
      }));
      msgs.push(...imageMessages);
    }

    const indexed = msgs.map((msg, index) => ({ msg, index }));
    indexed.sort((a, b) => {
      if (a.msg.createdAt === b.msg.createdAt) {
        return a.index - b.index;
      }
      return a.msg.createdAt - b.msg.createdAt;
    });

    return indexed.map((item) => item.msg);
  })();

  const imageGenerationLookup = useMemo(
    () => new Map(imageGenerations.map((entry) => [entry.id, entry])),
    [imageGenerations],
  );

  const { previewPersona, previewLorebook, previewLorebookEntries } = useMemo(() => {
    const data: {
      persona: PreviewPersona | null;
      lorebook: PreviewLorebook | null;
      entries: PreviewLorebookEntry[];
    } = {
      persona: null,
      lorebook: null,
      entries: [],
    };

    if (!session?.messages?.length) {
      return {
        previewPersona: data.persona,
        previewLorebook: data.lorebook,
        previewLorebookEntries: data.entries,
      };
    }

    const entriesById = new Map<string, PreviewLorebookEntry>();

    for (const message of session.messages) {
      if (!message.toolResults?.length) continue;
      for (const result of message.toolResults) {
        const payload = result.result as any;
        if (!payload || typeof payload !== "object") continue;

        if (payload.persona && typeof payload.persona === "object") {
          data.persona = payload.persona as PreviewPersona;
        }

        if (payload.lorebook && typeof payload.lorebook === "object") {
          data.lorebook = payload.lorebook as PreviewLorebook;
        }

        if (Array.isArray(payload.entries)) {
          for (const entry of payload.entries) {
            if (entry?.id) entriesById.set(entry.id, entry as PreviewLorebookEntry);
          }
        }

        if (payload.entry && typeof payload.entry === "object" && payload.entry.id) {
          entriesById.set(payload.entry.id, payload.entry as PreviewLorebookEntry);
        }
      }
    }

    const allEntries = Array.from(entriesById.values());
    const lorebookId = data.lorebook?.id;
    const filteredEntries = lorebookId
      ? allEntries.filter((entry) => entry.lorebookId === lorebookId)
      : allEntries;

    filteredEntries.sort((a, b) => {
      const orderA = a.displayOrder ?? 0;
      const orderB = b.displayOrder ?? 0;
      if (orderA !== orderB) return orderA - orderB;
      return (a.title ?? "").localeCompare(b.title ?? "");
    });

    data.entries = filteredEntries;

    const fallbackPersona =
      data.persona ||
      (session?.draft?.name || session?.draft?.description || session?.draft?.avatarPath
        ? {
            id:
              session?.creationMode === "edit" && session?.targetType === "persona"
                ? (session.targetId ?? undefined)
                : undefined,
            title: session?.draft?.name ?? undefined,
            description: session?.draft?.description ?? session?.draft?.definition ?? undefined,
            avatarPath: session?.draft?.avatarPath ?? undefined,
          }
        : null);

    const fallbackLorebook =
      data.lorebook ||
      (session?.draft?.name
        ? {
            id:
              session?.creationMode === "edit" && session?.targetType === "lorebook"
                ? (session.targetId ?? undefined)
                : undefined,
            name: session.draft.name,
          }
        : null);

    return {
      previewPersona: fallbackPersona,
      previewLorebook: fallbackLorebook,
      previewLorebookEntries: data.entries,
    };
  }, [session?.messages, session?.draft]);

  const previewTitle = showConfirmation
    ? activeGoal === "persona"
      ? "Ready to Save Persona?"
      : activeGoal === "lorebook"
        ? "Ready to Save Lorebook?"
        : "Ready to Save?"
    : activeGoal === "persona"
      ? "Persona Preview"
      : activeGoal === "lorebook"
        ? "Lorebook Preview"
        : "Character Preview";

  const handleOpenPersona = useCallback(async () => {
    const personaId =
      session?.creationMode === "edit" && session?.targetType === "persona"
        ? session.targetId
        : previewPersona?.id;
    if (!personaId) return;

    if (session?.creationMode === "edit" && session?.targetType === "persona") {
      try {
        await invoke("creation_helper_complete", {
          sessionId: session.id,
        });
      } catch (err) {
        console.error("Failed to save persona edit:", err);
        setError(resolveErrorMessage(err, "Failed to save persona changes."));
        return;
      }
    }

    navigate(`/settings/personas/${personaId}/edit`);
  }, [
    navigate,
    previewPersona?.id,
    session?.creationMode,
    session?.id,
    session?.targetId,
    session?.targetType,
    resolveErrorMessage,
  ]);

  const handleOpenLorebook = useCallback(async () => {
    const lorebookId =
      session?.creationMode === "edit" && session?.targetType === "lorebook"
        ? session.targetId
        : previewLorebook?.id;
    if (!lorebookId) return;

    if (session?.creationMode === "edit" && session?.targetType === "lorebook") {
      try {
        await invoke("creation_helper_complete", {
          sessionId: session.id,
        });
      } catch (err) {
        console.error("Failed to save lorebook edit:", err);
        setError(resolveErrorMessage(err, "Failed to save lorebook changes."));
        return;
      }
    }

    navigate(`/library/lorebooks/${lorebookId}`);
  }, [
    navigate,
    previewLorebook?.id,
    session?.creationMode,
    session?.id,
    session?.targetId,
    session?.targetType,
    resolveErrorMessage,
  ]);

  return (
    <div className="flex h-screen flex-col bg-surface">
      <TopNav
        currentPath="/create/character/helper"
        onBackOverride={handleBack}
        titleOverride={`AI ${goalLabel} Creator`}
        rightAction={
          activeGoal ? (
            <button
              onClick={() => {
                setShowConfirmation(false);
                setShowPreview(true);
              }}
              className={cn(
                "flex items-center gap-2 px-3 py-1.5 rounded-lg transition-all",
                "bg-fg/5 border border-fg/10 text-fg/70 hover:text-fg hover:bg-fg/10",
                "active:scale-95",
              )}
            >
              <Eye className="h-4 w-4" />
              <span className="text-xs font-medium">Preview</span>
            </button>
          ) : null
        }
      />

      {/* Messages Container */}
      <main className="flex-1 overflow-y-auto px-4 pt-[calc(72px+env(safe-area-inset-top))] pb-32">
        <div className="mx-auto max-w-2xl space-y-4 py-4">
          <div className="flex justify-center">
            <div className="rounded-full border border-fg/10 bg-fg/5 px-3 py-1 text-[10px] uppercase tracking-wider text-fg/50">
              {goalLabel} Mode
            </div>
          </div>
          {/* Welcome Message */}
          {!session?.messages.length && (
            <motion.div
              {...animations.fadeIn}
              className="flex flex-col items-center justify-center py-16 text-center"
            >
              <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-linear-to-br from-danger/20 to-warning/80/20 border border-danger/30">
                <Sparkles className="h-8 w-8 text-danger" />
              </div>
              <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-2")}>
                AI {goalLabel} Creator
              </h2>
              <p className="text-fg/60 text-sm max-w-xs">
                {!smartToolSelection
                  ? "Tell me what you'd like to create and I'll help you build it."
                  : activeGoal === "persona"
                    ? "I'll help you create a persona through conversation. Tell me who you want to be."
                    : activeGoal === "lorebook"
                      ? "I'll help you craft a lorebook through conversation. Tell me about your world."
                      : "I'll help you create a character through conversation. Just tell me what you have in mind!"}
              </p>
              <div className="mt-4 flex items-center gap-1">
                <Loader2 className="h-4 w-4 text-fg/40 animate-spin" />
                <span className="text-xs text-fg/40">Starting...</span>
              </div>
            </motion.div>
          )}

          {/* Messages */}
          <AnimatePresence mode="popLayout">
            {displayMessages.map((message) => {
              const imageEntry = imageGenerationLookup.get(message.id);
              const isUser = message.role === "user";
              const isSystem = message.role === "system";
              const alignClass = imageEntry
                ? "justify-start"
                : isSystem
                  ? "justify-center"
                  : isUser
                    ? "justify-end"
                    : "justify-start";
              return (
                <motion.div
                  key={message.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  className={cn("flex", alignClass)}
                >
                  <div
                    className={cn(
                      "max-w-[85%] rounded-2xl px-4 py-3",
                      imageEntry
                        ? "bg-fg/5 border border-fg/10 text-fg/90"
                        : isUser
                          ? "bg-fg/10 text-fg"
                          : "bg-fg/5 border border-fg/10 text-fg/90",
                    )}
                  >
                    {imageEntry ? (
                      <div className="flex flex-col items-center gap-2">
                        {imageEntry.status === "pending" ? (
                          <>
                            <div className="h-36 w-36 rounded-2xl border border-fg/10 bg-fg/5 flex items-center justify-center">
                              <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
                            </div>
                          </>
                        ) : imageEntry.status === "error" ? (
                          <>
                            <div className="h-36 w-36 rounded-2xl border border-danger/30 bg-danger/10 flex items-center justify-center">
                              <span className="text-xs text-danger">Generation failed</span>
                            </div>
                          </>
                        ) : imageEntry.imageId ? (
                          <button
                            type="button"
                            onClick={() =>
                              setImagePreview({
                                id: imageEntry.imageId as string,
                                label: "Generated image",
                              })
                            }
                            className="group flex flex-col items-center gap-2"
                          >
                            <GeneratedImagePreview
                              sessionId={session?.id ?? ""}
                              imageId={imageEntry.imageId}
                              label="Image ready"
                              size="sm"
                              className="transition-transform group-hover:scale-[1.01]"
                            />
                          </button>
                        ) : (
                          <span className="text-xs text-fg/40">Image unavailable</span>
                        )}
                      </div>
                    ) : (
                      <>
                        {/* Message Content */}
                        {(() => {
                          let displayText = message.content;

                          if (messageDisplayContent[message.id]) {
                            displayText = messageDisplayContent[message.id];
                          } else {
                            const separator = "\n\n---\n";
                            const sepIndex = displayText.indexOf(separator);
                            if (sepIndex !== -1) {
                              displayText = displayText.substring(0, sepIndex).trim();
                            }
                          }

                          if (message.id === "__streaming__" && !displayText.trim()) {
                            return <TypingIndicator />;
                          }

                          return (
                            <MarkdownRenderer
                              content={displayText}
                              className={cn(
                                "text-sm leading-relaxed",
                                message.role === "user" ? "text-fg" : "text-fg/90",
                              )}
                            />
                          );
                        })()}

                        {/* References & Attachments Display */}
                        {(() => {
                          // 1. References
                          let refs = messageReferences[message.id];
                          if (!refs || refs.length === 0) {
                            // Parse reference names and IDs from content as fallback
                            const refPattern =
                              /\[Referenced (Character|Persona): "([^"]+)" \(id:([^)]+)\)\]/g;
                            const matches = [...message.content.matchAll(refPattern)];
                            if (matches.length > 0) {
                              refs = matches.map((match) => ({
                                type: match[1].toLowerCase() as "character" | "persona",
                                id: match[3],
                                name: match[2],
                              }));
                            }
                          }

                          // 2. Uploaded Images
                          const imgPattern = /\[Uploaded Image: "([^"]+)" \(id:([^)]+)\)\]/g;
                          const imgMatches = [...message.content.matchAll(imgPattern)];
                          const images = imgMatches.map((match) => ({
                            filename: match[1],
                            id: match[2],
                          }));

                          if ((!refs || refs.length === 0) && images.length === 0) return null;

                          return (
                            <div className="mt-2 flex flex-wrap gap-1.5">
                              {/* References */}
                              {refs?.map((ref) => (
                                <div
                                  key={ref.id}
                                  className={cn(
                                    "flex items-center gap-1.5 px-2 py-1",
                                    "rounded-full text-xs",
                                    ref.type === "character"
                                      ? "bg-secondary/20 text-secondary"
                                      : "bg-warning/20 text-warning",
                                  )}
                                >
                                  <ReferenceAvatar
                                    type={ref.type}
                                    id={ref.id}
                                    avatarPath={ref.avatarPath || entityAvatars[ref.id]}
                                    name={ref.name}
                                    size="sm"
                                  />
                                  <span>{ref.name}</span>
                                </div>
                              ))}

                              {/* Images */}
                              {images.map((img) => (
                                <ImageThumbnail
                                  key={img.id}
                                  sessionId={session?.id as string}
                                  imageId={img.id}
                                  filename={img.filename}
                                  localCache={localImageCache}
                                />
                              ))}
                            </div>
                          );
                        })()}

                        {/* Tool Calls Display */}
                        {(() => {
                          const toolResults = message.toolResults || [];
                          const toolCalls = message.toolCalls || [];
                          const toolResultsById = new Map(
                            toolResults.map((result) => [result.toolCallId, result]),
                          );
                          const toolCallIds = new Set(toolCalls.map((call) => call.id));
                          const toolEntries = toolCalls.map((call) => ({
                            call,
                            result: toolResultsById.get(call.id),
                          }));
                          for (const result of toolResults) {
                            if (!toolCallIds.has(result.toolCallId)) {
                              toolEntries.push({
                                call: {
                                  id: result.toolCallId,
                                  name: "Unknown Tool",
                                  arguments: {},
                                },
                                result,
                              });
                            }
                          }

                          if (toolEntries.length === 0) return null;

                          return (
                            <div className="mt-3 space-y-1.5 border-t border-fg/10 pt-3">
                              {toolEntries.map(({ call, result }) => {
                                const displayName = getToolDisplayName(call.name || "Unknown Tool");
                                if (!result) {
                                  return (
                                    <div
                                      key={call.id}
                                      className={cn(
                                        "w-full flex items-center gap-2 text-xs rounded-lg px-2 py-1.5 text-left",
                                        "bg-fg/5 text-fg/60 border border-fg/10",
                                      )}
                                    >
                                      <Loader2 className="h-3 w-3 shrink-0 animate-spin" />
                                      <span className="truncate flex-1">{displayName}</span>
                                    </div>
                                  );
                                }

                                return (
                                  <button
                                    key={call.id}
                                    onClick={() => {
                                      setSelectedTool({ call, result });
                                      setShowToolDetail(true);
                                    }}
                                    className={cn(
                                      "w-full flex items-center gap-2 text-xs rounded-lg px-2 py-1.5 transition-all text-left group",
                                      result.success
                                        ? "bg-accent/10 text-accent/80 hover:bg-accent/20"
                                        : "bg-danger/10 text-danger hover:bg-danger/20",
                                    )}
                                  >
                                    {result.success ? (
                                      <Check className="h-3 w-3 shrink-0" />
                                    ) : (
                                      <span className="h-3 w-3 shrink-0">✗</span>
                                    )}
                                    <span className="truncate flex-1">{displayName}</span>
                                    <span className="text-[10px] opacity-0 group-hover:opacity-100 transition-opacity">
                                      View Details
                                    </span>
                                  </button>
                                );
                              })}
                            </div>
                          );
                        })()}
                      </>
                    )}
                  </div>
                </motion.div>
              );
            })}
          </AnimatePresence>

          {/* Regeneration Button */}
          {session &&
            !sending &&
            session.messages.length > 0 &&
            session.messages[session.messages.length - 1].role === "assistant" && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex justify-center mb-6"
              >
                <button
                  onClick={handleRegenerate}
                  className={cn(
                    "flex items-center gap-2 px-4 py-2",
                    "bg-fg/5 border border-fg/10 text-fg/60 hover:text-fg hover:bg-fg/10 transition-colors",
                    radius.full,
                    typography.bodySmall.size,
                  )}
                >
                  <RefreshCw className="h-4 w-4" />
                  <span>Regenerate Response</span>
                </button>
              </motion.div>
            )}

          <div ref={messagesEndRef} />
        </div>
      </main>

      {/* Footer Input Area */}
      <div className="fixed bottom-0 left-0 right-0 pb-[env(safe-area-inset-bottom)]">
        <CreationHelperFooter
          draft={inputValue}
          setDraft={setInputValue}
          error={error}
          sending={sending}
          onSendMessage={handleSend}
          onRetry={() => {
            if (lastActionRef.current === "regenerate") {
              handleRegenerate();
            } else {
              handleSend();
            }
          }}
          onAbort={handleAbort}
          pendingAttachments={pendingAttachments}
          onAddAttachment={(attachment) => {
            setPendingAttachments((prev) => [...prev, attachment]);
            setLocalImageCache((prev) => ({ ...prev, [attachment.id]: attachment.data }));
          }}
          onRemoveAttachment={(id) => {
            setPendingAttachments((prev) => prev.filter((a) => a.id !== id));
          }}
          references={references}
          onRemoveReference={(id) => {
            setReferences((prev) => prev.filter((r) => r.id !== id));
          }}
          onOpenReferenceSelector={(type) => {
            setReferenceSelectorType(type);
            setShowReferenceSelector(true);
          }}
        />
      </div>

      {/* Reference Selector */}
      <ReferenceSelector
        isOpen={showReferenceSelector}
        onClose={() => setShowReferenceSelector(false)}
        type={referenceSelectorType}
        onSelect={(ref) => {
          setReferences((prev) => [...prev, ref]);
        }}
        existingRefs={references}
      />

      {/* Preview Bottom Sheet */}
      <BottomMenu
        isOpen={showPreview}
        onClose={() => {
          setShowPreview(false);
          setShowConfirmation(false);
        }}
        title={previewTitle}
      >
        {activeGoal === "character" && session?.draft && (
          <div className="space-y-4">
            <CharacterPreviewCard draft={session.draft} sessionId={session.id} />

            <MenuSection>
              <MenuButton
                icon={Check}
                title={
                  session.creationMode === "edit" && session.targetType === "character"
                    ? "Save Character Changes"
                    : "Use This Character"
                }
                description={
                  session.creationMode === "edit" && session.targetType === "character"
                    ? "Apply updates to existing character"
                    : "Save and start chatting"
                }
                color="from-accent to-accent/80"
                onClick={handleUseCharacter}
              />
              <MenuButton
                icon={RefreshCw}
                title="Keep Editing"
                description="Continue the conversation"
                color="from-info to-info/80"
                onClick={() => {
                  setShowPreview(false);
                  setShowConfirmation(false);
                }}
              />
              <MenuButton
                icon={PenLine}
                title="Edit Manually"
                description="Fine-tune in the editor"
                color="from-warning to-warning/80"
                onClick={handleEditManually}
              />
            </MenuSection>
          </div>
        )}

        {activeGoal === "persona" && (
          <div className="space-y-4">
            <PersonaPreviewCard persona={previewPersona} sessionId={session?.id} />

            <MenuSection>
              <MenuButton
                icon={User}
                title={
                  session?.creationMode === "edit" && session?.targetType === "persona"
                    ? "Save Persona Changes"
                    : "Open Persona"
                }
                description={
                  session?.creationMode === "edit" && session?.targetType === "persona"
                    ? "Apply updates to existing persona"
                    : previewPersona?.id
                      ? "Review and edit your persona"
                      : "Create a persona first"
                }
                color="from-accent to-accent/80"
                onClick={handleOpenPersona}
                disabled={
                  session?.creationMode === "edit" && session?.targetType === "persona"
                    ? !session?.targetId
                    : !previewPersona?.id
                }
              />
              <MenuButton
                icon={RefreshCw}
                title="Keep Editing"
                description="Continue the conversation"
                color="from-info to-info/80"
                onClick={() => {
                  setShowPreview(false);
                  setShowConfirmation(false);
                }}
              />
            </MenuSection>
          </div>
        )}

        {activeGoal === "lorebook" && (
          <div className="space-y-4">
            <LorebookPreviewCard lorebook={previewLorebook} entries={previewLorebookEntries} />

            <MenuSection>
              <MenuButton
                icon={BookOpen}
                title={
                  session?.creationMode === "edit" && session?.targetType === "lorebook"
                    ? "Save Lorebook Changes"
                    : "Open Lorebook"
                }
                description={
                  session?.creationMode === "edit" && session?.targetType === "lorebook"
                    ? "Apply updates to existing lorebook"
                    : previewLorebook?.id
                      ? "Review entries in the library"
                      : "Create a lorebook first"
                }
                color="from-accent to-accent/80"
                onClick={handleOpenLorebook}
                disabled={
                  session?.creationMode === "edit" && session?.targetType === "lorebook"
                    ? !session?.targetId
                    : !previewLorebook?.id
                }
              />
              <MenuButton
                icon={RefreshCw}
                title="Keep Editing"
                description="Continue the conversation"
                color="from-info to-info/80"
                onClick={() => {
                  setShowPreview(false);
                  setShowConfirmation(false);
                }}
              />
            </MenuSection>
          </div>
        )}
      </BottomMenu>

      {/* Tool Detail Bottom Sheet */}
      <BottomMenu
        isOpen={showToolDetail}
        onClose={() => setShowToolDetail(false)}
        title={selectedTool ? getToolDisplayName(selectedTool.call.name) : "Tool Usage Details"}
      >
        {selectedTool && (
          <div className="space-y-6 pb-6">
            <div className="flex items-center gap-2">
              <div
                className={cn(
                  "p-2 rounded-lg",
                  selectedTool.result.success
                    ? "bg-accent/15 text-accent"
                    : "bg-danger/15 text-danger",
                )}
              >
                {selectedTool.result.success ? (
                  <Check className="h-5 w-5" />
                ) : (
                  <span className="text-lg font-bold">✗</span>
                )}
              </div>
              <div>
                <h3
                  className={cn(typography.h2.size, typography.h2.weight, "text-fg text-base")}
                >
                  {selectedTool.result.success ? "Execution Successful" : "Execution Failed"}
                </h3>
                <p className="text-fg/40 text-[10px] uppercase tracking-wider font-bold">
                  Tool: {selectedTool.call.name}
                </p>
              </div>
            </div>

            <div className="space-y-4">
              <div className="space-y-2">
                <h4 className="text-[10px] font-bold text-fg/30 uppercase tracking-wider px-1">
                  Model Input (Arguments)
                </h4>
                <div className="bg-surface-el/40 rounded-xl p-3 border border-fg/5 overflow-x-auto">
                  <pre className="text-xs text-info font-mono leading-relaxed">
                    {JSON.stringify(selectedTool.call.arguments, null, 2)}
                  </pre>
                </div>
              </div>

              <div className="space-y-2">
                <h4 className="text-[10px] font-bold text-fg/30 uppercase tracking-wider px-1">
                  Tool Output (Result)
                </h4>
                <div className="bg-surface-el/40 rounded-xl p-3 border border-fg/5 overflow-x-auto">
                  <pre
                    className={cn(
                      "text-xs font-mono leading-relaxed",
                      selectedTool.result.success ? "text-accent/80" : "text-danger",
                    )}
                  >
                    {JSON.stringify(selectedTool.result.result, null, 2)}
                  </pre>
                </div>
              </div>
            </div>
          </div>
        )}
      </BottomMenu>

      <BottomMenu
        isOpen={!!imagePreview}
        onClose={() => setImagePreview(null)}
        title={imagePreview?.label ?? "Generated Image"}
      >
        {imagePreview && session?.id && (
          <div className="flex justify-center py-6">
            <GeneratedImagePreview
              sessionId={session.id}
              imageId={imagePreview.id}
              label={imagePreview.label}
              size="lg"
            />
          </div>
        )}
      </BottomMenu>
    </div>
  );
}
