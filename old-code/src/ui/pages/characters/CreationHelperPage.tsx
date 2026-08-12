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
} from "lucide-react";
import { TopNav } from "../../components/App";
import { cn, typography, animations, radius } from "../../design-tokens";
import { BottomMenu, MenuButton, MenuSection } from "../../components";
import { toast } from "../../components/toast";
import { MarkdownRenderer } from "../chats/components/MarkdownRenderer";
import { CharacterPreviewCard, PersonaPreviewCard } from "./components";
import { CreationHelperFooter } from "./components/CreationHelperFooter";
import { ReferenceSelector, ReferenceAvatar, Reference } from "./components/ReferenceSelector";
import { convertToImageUrl } from "../../../core/storage/images";
import { listCharacters, listPersonas } from "../../../core/storage/repo";
import { isRenderableImageUrl } from "../../../core/utils/image";
import { useI18n } from "../../../core/i18n/context";
import type { TranslationKey } from "../../../core/i18n/context";

interface CreationMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  toolCalls: ToolCall[];
  toolResults: ToolResult[];
  blocks?: CreationBlock[];
  createdAt: number;
}

type CreationBlock =
  | { kind: "text"; content: string }
  | { kind: "tool"; toolCallId: string };

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

interface CreationSession {
  id: string;
  messages: CreationMessage[];
  draft: DraftCharacter;
  draftHistory: DraftCharacter[];
  creationGoal: "character" | "persona";
  creationMode: "create" | "edit";
  targetType?: "character" | "persona" | null;
  targetId?: string | null;
  status: "active" | "previewShown" | "completed" | "cancelled";
  createdAt: number;
  updatedAt: number;
}

interface CreationHelperUpdatePayload {
  sessionId: string;
  draft: DraftCharacter;
  status: CreationSession["status"];
  messages: CreationMessage[];
  activeToolCalls?: ToolCall[] | null;
  activeToolResults?: ToolResult[] | null;
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
  assetId?: string | null;
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
  const { t } = useI18n();
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
          if (img.data && isRenderableImageUrl(img.data)) {
            setImageUrl(img.data);
          } else if (img.assetId) {
            setImageUrl((await convertToImageUrl(img.assetId)) ?? null);
          } else {
            setImageUrl(null);
          }
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
        <span>{t("characters.creationHelper.failedToLoad")}</span>
      </div>
    );

  return (
    <div className="group relative h-24 w-24 overflow-hidden rounded-lg border border-fg/10 bg-surface-el/20">
      <img
        src={imageUrl}
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
  size?: "xs" | "sm" | "md" | "lg";
  className?: string;
}) {
  const { t } = useI18n();
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const sizeClass =
    size === "xs"
      ? "h-20 w-20"
      : size === "lg"
        ? "h-72 w-72"
        : size === "sm"
          ? "h-36 w-36"
          : "h-40 w-40";

  useEffect(() => {
    let active = true;
    const fetchImage = async () => {
      try {
        const img = await invoke<UploadedImage | null>("creation_helper_get_uploaded_image", {
          sessionId,
          imageId,
        });
        if (active && img) {
          if (img.data && isRenderableImageUrl(img.data)) {
            setImageUrl(img.data);
          } else if (img.assetId) {
            setImageUrl((await convertToImageUrl(img.assetId)) ?? null);
          } else {
            setImageUrl(null);
          }
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
          "overflow-hidden rounded-lg border border-fg/10 bg-fg/5 flex items-center justify-center",
          size === "lg" && "rounded-2xl",
        )}
      >
        {loading ? (
          <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
        ) : imageUrl ? (
          <img src={imageUrl} alt={label} className="h-full w-full object-cover" />
        ) : (
          <span className="text-xs text-fg/40">{t("characters.creationHelper.failedToLoad")}</span>
        )}
      </div>
    </div>
  );
}


function TypingIndicator() {
  const { t } = useI18n();
  return (
    <motion.div
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2 }}
      className="flex items-center gap-2"
      aria-label={t("characters.creationHelper.assistantTyping")}
      aria-live="polite"
    >
      <div className="flex items-center gap-1">
        <span className="typing-dot" />
        <span className="typing-dot" style={{ animationDelay: "0.2s" }} />
        <span className="typing-dot" style={{ animationDelay: "0.4s" }} />
      </div>
    </motion.div>
  );
}

export function CreationHelperPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const [searchParams] = useSearchParams();
  const creationGoalParam = searchParams.get("goal");
  const sessionIdParam = searchParams.get("sessionId");
  const modeParam = searchParams.get("mode");
  const targetTypeParam = searchParams.get("targetType");
  const targetIdParam = searchParams.get("targetId");
  const creationGoal: CreationSession["creationGoal"] =
    creationGoalParam === "persona" ? "persona" : "character";
  const creationMode: CreationSession["creationMode"] = modeParam === "edit" ? "edit" : "create";
  const targetType: CreationSession["targetType"] =
    targetTypeParam === "character" || targetTypeParam === "persona"
      ? targetTypeParam
      : null;
  const targetId = targetIdParam?.trim() || null;
  const [session, setSession] = useState<CreationSession | null>(null);
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
  const [activeToolResults, setActiveToolResults] = useState<ToolResult[]>([]);
  const [streamingSegments, setStreamingSegments] = useState<string[]>([]);
  const [liveSteps, setLiveSteps] = useState<
    Array<{
      stepIndex: number;
      name: string;
      arguments: unknown;
      status: "running" | "completed" | "failed";
      result?: unknown;
    }>
  >([]);
  const [liveBlocks, setLiveBlocks] = useState<
    Array<
      | { kind: "text"; content: string }
      | { kind: "tool"; stepIndex: number }
    >
  >([]);
  const liveBlocksRef = useRef(liveBlocks);
  liveBlocksRef.current = liveBlocks;
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamUnlistenRef = useRef<(() => void) | null>(null);
  const streamingContentRef = useRef<string>("");
  const previewAutoOpenedRef = useRef<Set<number>>(new Set());
  const previewDismissedRef = useRef<boolean>(false);
  const streamingSegmentsRef = useRef<string[]>([]);
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

  const resetStreamingTransient = useCallback(() => {
    setStreamingContent("");
    streamingContentRef.current = "";
    setStreamingReasoning("");
    setActiveTools([]);
    setActiveToolResults([]);
    setStreamingSegments([]);
    streamingSegmentsRef.current = [];
  }, []);

  const flushStreamingSegment = useCallback(() => {
    const buf = streamingContentRef.current;
    if (buf.trim()) {
      streamingSegmentsRef.current = [...streamingSegmentsRef.current, buf];
      setStreamingSegments(streamingSegmentsRef.current);
    }
    streamingContentRef.current = "";
    setStreamingContent("");
  }, []);

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
  const goalLabel =
    activeGoal === "persona"
      ? t("characters.creationHelper.goalPersona")
      : t("characters.creationHelper.goalCharacter");

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
        const smartSelection = true;

        let resumedSession: CreationSession | null = null;
        const requestedSessionId = sessionIdParam?.trim() || null;

        if (requestedSessionId) {
          resumedSession = await invoke<CreationSession | null>("creation_helper_get_session", {
            sessionId: requestedSessionId,
          });
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
                  : "Hi! I want to create a new character."
                : "Hi! I want to create something new.",
          uploadedImages: null,
        });
        setSession(greetingSession);
      } catch (err) {
        console.error("Failed to start creation helper:", err);
        setError(t("characters.creationHelper.startFailed"));
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
    resetStreamingTransient();
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
    const generationCalls = activeTools.filter((call) => isImageGenCall(call.name));
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
      const resultSources = [
        ...session.messages.map((message) => ({
          toolCalls: message.toolCalls ?? [],
          toolResults: message.toolResults ?? [],
          createdAt: message.createdAt ?? Date.now(),
        })),
        {
          toolCalls: activeTools,
          toolResults: activeToolResults,
          createdAt: Date.now(),
        },
      ];

      for (const message of resultSources) {
        if (!message.toolResults?.length || !message.toolCalls?.length) continue;
        for (const result of message.toolResults) {
          const toolCall = message.toolCalls.find((call) => call.id === result.toolCallId);
          if (!toolCall || !isImageGenCall(toolCall.name)) continue;
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
  }, [session?.messages, activeTools, activeToolResults]);

  // Listen for updates from backend
  useEffect(() => {
    const unlisten = listen("creation-helper-update", (event) => {
      const payload = event.payload as CreationHelperUpdatePayload;

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

        if (Array.isArray(payload.activeToolCalls)) {
          setActiveTools(payload.activeToolCalls);
        }
        if (Array.isArray(payload.activeToolResults)) {
          setActiveToolResults(payload.activeToolResults);
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [session?.id]);

  useEffect(() => {
    if (!session?.id) return;
    const unlistenStart = listen<{ sessionId: string }>(
      "creation-helper-turn-start",
      (event) => {
        if (event.payload?.sessionId === session.id) {
          setLiveSteps([]);
          setLiveBlocks([]);
          liveBlocksRef.current = [];
          previewAutoOpenedRef.current = new Set();
        }
      },
    );
    const unlistenStep = listen<{
      sessionId: string;
      stepIndex: number;
      name: string;
      arguments: unknown;
      status: "running" | "completed" | "failed";
      result?: unknown;
    }>("creation-helper-step", (event) => {
      const p = event.payload;
      if (!p || p.sessionId !== session.id) return;
      setLiveSteps((prev) => {
        const idx = prev.findIndex((s) => s.stepIndex === p.stepIndex);
        const next = idx >= 0 ? [...prev] : [...prev];
        const entry = {
          stepIndex: p.stepIndex,
          name: p.name,
          arguments: p.arguments,
          status: p.status,
          result: p.result,
        };
        if (idx >= 0) {
          next[idx] = entry;
        } else {
          next.push(entry);
          next.sort((a, b) => a.stepIndex - b.stepIndex);
        }
        return next;
      });
      if (p.status === "running") {
        const buf = streamingContentRef.current.trim();
        const blocks = [...liveBlocksRef.current];
        if (buf) {
          blocks.push({ kind: "text", content: buf });
          streamingContentRef.current = "";
          setStreamingContent("");
        }
        if (!blocks.some((b) => b.kind === "tool" && b.stepIndex === p.stepIndex)) {
          blocks.push({ kind: "tool", stepIndex: p.stepIndex });
        }
        liveBlocksRef.current = blocks;
        setLiveBlocks(blocks);
      }
      const resObj = p.result && typeof p.result === "object"
        ? (p.result as Record<string, unknown>)
        : null;
      if (resObj?.action === "show_preview" || resObj?.action === "request_confirmation") {
        const seen = previewAutoOpenedRef.current;
        if (!seen.has(p.stepIndex) && !previewDismissedRef.current) {
          seen.add(p.stepIndex);
          setShowPreview(true);
          if (resObj?.action === "request_confirmation") setShowConfirmation(true);
        }
      }
    });
    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStep.then((fn) => fn());
    };
  }, [session?.id]);

  useEffect(() => {
    if (!sending && (liveSteps.length > 0 || liveBlocks.length > 0)) {
      setLiveSteps([]);
      setLiveBlocks([]);
      liveBlocksRef.current = [];
    }
  }, [sending, liveSteps.length, liveBlocks.length]);

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
    resetStreamingTransient();
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
        } else if (payload.type === "creationSegmentBoundary") {
          flushStreamingSegment();
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
            t("characters.creationHelper.streamingError"),
          );
          setError(message);
          resetStreamingTransient();
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
          setError(t("characters.creationHelper.noResponse"));
          const snapshot = lastSendSnapshotRef.current;
          if (snapshot) {
            setInputValue((prev) => (prev.trim() ? prev : snapshot.draft));
            setReferences((prev) => (prev.length ? prev : snapshot.references));
            setPendingAttachments((prev) => (prev.length ? prev : snapshot.attachments));
          }
        }
      }
    } catch (err: any) {
      console.error("Failed to send message:", err);
      setError(resolveErrorMessage(err, t("characters.creationHelper.sendFailed")));

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
      resetStreamingTransient();
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
    resetStreamingTransient();
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
        } else if (payload.type === "creationSegmentBoundary") {
          flushStreamingSegment();
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
            t("characters.creationHelper.streamingError"),
          );
          setError(message);
          resetStreamingTransient();
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
          setError(t("characters.creationHelper.noResponse"));
        }
      }
    } catch (err: any) {
      console.error("Failed to regenerate:", err);
      setError(resolveErrorMessage(err, t("characters.creationHelper.regenerateFailed")));
    } finally {
      if (unlistenStream) {
        unlistenStream();
      }
      if (streamUnlistenRef.current === unlistenStream) {
        streamUnlistenRef.current = null;
      }
      setSending(false);
      resetStreamingTransient();
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
      setError(resolveErrorMessage(err, t("characters.creationHelper.saveCharacterFailed")));
    }
  }, [session, navigate]);

  const handleEditManually = useCallback(async () => {
    if (!session) return;
    try {
      const draft = await invoke<DraftCharacter>("creation_helper_complete", {
        sessionId: session.id,
      });
      navigate("/create/character", {
        state: { draftCharacter: draft },
      });
    } catch (err) {
      console.error("Failed to prepare character draft:", err);
      setError(resolveErrorMessage(err, t("characters.creationHelper.saveCharacterFailed")));
    }
  }, [session, navigate, resolveErrorMessage, t]);

  const handleSaveAndChat = useCallback(async () => {
    if (!session) return;
    try {
      const draft = await invoke<DraftCharacter>("creation_helper_complete", {
        sessionId: session.id,
      });
      navigate("/create/character", {
        state: { draftCharacter: draft, autoSaveAndChat: true },
      });
    } catch (err: any) {
      console.error("Failed to complete character:", err);
      setError(resolveErrorMessage(err, t("characters.creationHelper.saveCharacterFailed")));
    }
  }, [session, navigate, resolveErrorMessage]);

  const handleAbort = useCallback(() => {
    if (!session) return;
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current();
      streamUnlistenRef.current = null;
    }
    invoke("creation_helper_cancel", { sessionId: session.id })
      .then(() => {
        setSending(false);
        resetStreamingTransient();
        setError(t("characters.creationHelper.generationCancelled"));
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
    resetStreamingTransient();
    navigate(-1);
  };

  const getToolDisplayName = (toolName: string): string => {
    const names = {
      write_definition: "characters.creationHelper.toolNames.write_definition",
      write_scene: "characters.creationHelper.toolNames.write_scene",
      write_lore_entry: "characters.creationHelper.toolNames.write_lore_entry",
      set_name: "characters.creationHelper.toolNames.set_name",
      set_model: "characters.creationHelper.toolNames.set_model",
      set_prompt: "characters.creationHelper.toolNames.set_prompt",
      set_avatar_gradient: "characters.creationHelper.toolNames.set_avatar_gradient",
      attach_lorebooks: "characters.creationHelper.toolNames.attach_lorebooks",
      edit_scene: "characters.creationHelper.toolNames.edit_scene",
      edit_lore_entry: "characters.creationHelper.toolNames.edit_lore_entry",
      delete_scene: "characters.creationHelper.toolNames.delete_scene",
      delete_lore_entry: "characters.creationHelper.toolNames.delete_lore_entry",
      delete_persona: "characters.creationHelper.toolNames.delete_persona",
      delete_lorebook: "characters.creationHelper.toolNames.delete_lorebook",
      reorder_lore_entries: "characters.creationHelper.toolNames.reorder_lore_entries",
      generate_image: "characters.creationHelper.toolNames.generate_image",
      edit_avatar_image: "characters.creationHelper.toolNames.edit_avatar_image",
      use_uploaded_image: "characters.creationHelper.toolNames.use_uploaded_image",
      show_preview: "characters.creationHelper.toolNames.show_preview",
      request_confirmation: "characters.creationHelper.toolNames.request_confirmation",
      list_models: "characters.creationHelper.toolNames.list_models",
      list_prompts: "characters.creationHelper.toolNames.list_prompts",
      list_personas: "characters.creationHelper.toolNames.list_personas",
      list_lorebooks: "characters.creationHelper.toolNames.list_lorebooks",
      list_lore_entries: "characters.creationHelper.toolNames.list_lore_entries",
      set_character_name: "characters.creationHelper.toolNames.set_character_name",
      set_character_definition: "characters.creationHelper.toolNames.set_character_definition",
      set_character_description: "characters.creationHelper.toolNames.set_character_description",
      add_scene: "characters.creationHelper.toolNames.add_scene",
      update_scene: "characters.creationHelper.toolNames.update_scene",
      toggle_avatar_gradient: "characters.creationHelper.toolNames.toggle_avatar_gradient",
      set_default_model: "characters.creationHelper.toolNames.set_default_model",
      set_system_prompt: "characters.creationHelper.toolNames.set_system_prompt",
      use_uploaded_image_as_avatar: "characters.creationHelper.toolNames.use_uploaded_image_as_avatar",
      use_uploaded_image_as_chat_background:
        "characters.creationHelper.toolNames.use_uploaded_image_as_chat_background",
      generate_avatar: "characters.creationHelper.toolNames.generate_avatar",
      upsert_persona: "characters.creationHelper.toolNames.upsert_persona",
      use_uploaded_image_as_persona_avatar:
        "characters.creationHelper.toolNames.use_uploaded_image_as_persona_avatar",
      get_default_persona: "characters.creationHelper.toolNames.get_default_persona",
      upsert_lorebook: "characters.creationHelper.toolNames.upsert_lorebook",
      list_lorebook_entries: "characters.creationHelper.toolNames.list_lorebook_entries",
      get_lorebook_entry: "characters.creationHelper.toolNames.get_lorebook_entry",
      upsert_lorebook_entry: "characters.creationHelper.toolNames.upsert_lorebook_entry",
      delete_lorebook_entry: "characters.creationHelper.toolNames.delete_lorebook_entry",
      create_blank_lorebook_entry: "characters.creationHelper.toolNames.create_blank_lorebook_entry",
      reorder_lorebook_entries: "characters.creationHelper.toolNames.reorder_lorebook_entries",
      list_character_lorebooks: "characters.creationHelper.toolNames.list_character_lorebooks",
      set_character_lorebooks: "characters.creationHelper.toolNames.set_character_lorebooks",
    } satisfies Record<string, TranslationKey>;
    const key = names[toolName as keyof typeof names];
    return key ? t(key) : toolName;
  };

  const isImageGenCall = (name: string | undefined) =>
    name === "generate_image" || name === "edit_avatar_image";

  const displayMessages = (() => {
    if (!session) return [];

    const msgs = [...session.messages];
    const lastMessage = msgs[msgs.length - 1];
    const hasCommittedAssistantTail =
      lastMessage?.role === "assistant" &&
      (!!lastMessage.content?.trim() ||
        (lastMessage.toolCalls?.length ?? 0) > 0 ||
        (lastMessage.toolResults?.length ?? 0) > 0);
    const hasTransientAssistantState =
      !!streamingContent.trim() ||
      streamingSegments.length > 0 ||
      liveSteps.length > 0 ||
      liveBlocks.length > 0 ||
      activeTools.length > 0 ||
      activeToolResults.length > 0;

    if (sending && !hasCommittedAssistantTail && !hasTransientAssistantState) {
      msgs.push({
        id: "__placeholder__",
        role: "assistant",
        content: "",
        toolCalls: [],
        toolResults: [],
        createdAt: Date.now(),
      });
    }

    if (sending && !hasCommittedAssistantTail && hasTransientAssistantState) {
      const stepById = new Map(liveSteps.map((s) => [s.stepIndex, s]));
      const liveCalls = liveSteps.map((s) => ({
        id: `__live_${s.stepIndex}`,
        name: s.name,
        arguments: s.arguments,
      }));
      const liveResults = liveSteps
        .filter((s) => s.status !== "running")
        .map((s) => ({
          toolCallId: `__live_${s.stepIndex}`,
          result: s.result ?? {},
          success: s.status === "completed",
        }));
      const blocks: CreationBlock[] = liveBlocks.map((b) => {
        if (b.kind === "text") return { kind: "text", content: b.content };
        const step = stepById.get(b.stepIndex);
        return {
          kind: "tool",
          toolCallId: step ? `__live_${step.stepIndex}` : `__live_unknown_${b.stepIndex}`,
        };
      });
      if (streamingContent.trim()) {
        blocks.push({ kind: "text", content: streamingContent });
      }
      msgs.push({
        id: "__streaming__",
        role: "assistant",
        content: streamingContent || "",
        toolCalls: liveCalls.length > 0 ? liveCalls : activeTools,
        toolResults: liveResults.length > 0 ? liveResults : activeToolResults,
        blocks: blocks.length > 0 ? blocks : undefined,
        createdAt: Date.now(),
      });
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

  const previewPersona = useMemo(() => {
    const data: { persona: PreviewPersona | null } = {
      persona: null,
    };

    if (!session?.messages?.length) {
      return data.persona;
    }

    for (const message of session.messages) {
      if (!message.toolResults?.length) continue;
      for (const result of message.toolResults) {
        const payload = result.result as any;
        if (!payload || typeof payload !== "object") continue;

        if (payload.persona && typeof payload.persona === "object") {
          data.persona = payload.persona as PreviewPersona;
        }
      }
    }

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

    return fallbackPersona;
  }, [session?.messages, session?.draft]);

  const previewTitle = showConfirmation
    ? activeGoal === "persona"
      ? t("characters.creationHelper.readyToSavePersona")
      : t("characters.creationHelper.readyToSave")
    : activeGoal === "persona"
      ? t("characters.creationHelper.personaPreview")
      : t("characters.creationHelper.characterPreview");

  const handleOpenPersona = useCallback(async () => {
    if (!session) return;

    if (session.creationMode === "edit" && session.targetType === "persona") {
      const personaId = session.targetId;
      if (!personaId) return;
      try {
        await invoke("creation_helper_complete", {
          sessionId: session.id,
        });
      } catch (err) {
        console.error("Failed to save persona edit:", err);
        setError(resolveErrorMessage(err, t("characters.creationHelper.savePersonaChangesFailed")));
        return;
      }
      navigate(`/personas/${personaId}/edit`);
      return;
    }

    try {
      const draft = await invoke<DraftCharacter>("creation_helper_complete", {
        sessionId: session.id,
      });
      navigate("/create/persona", {
        state: { draftPersona: draft },
      });
    } catch (err) {
      console.error("Failed to complete persona:", err);
      setError(resolveErrorMessage(err, t("characters.creationHelper.savePersonaFailed")));
    }
  }, [
    navigate,
    session,
    resolveErrorMessage,
  ]);

  return (
    <div className="flex h-screen flex-col bg-surface">
      <TopNav
        currentPath="/create/character/helper"
        onBackOverride={handleBack}
        titleOverride={t("characters.creationHelper.creatorTitle", { goal: goalLabel })}
        rightAction={
          activeGoal ? (
            <button
              onClick={() => {
                setShowConfirmation(false);
                setShowPreview(true);
                previewDismissedRef.current = false;
              }}
              className={cn(
                "flex items-center gap-2 px-3 py-1.5 rounded-lg transition-all",
                "bg-fg/5 border border-fg/10 text-fg/70 hover:text-fg hover:bg-fg/10",
                "active:scale-95",
              )}
            >
              <Eye className="h-4 w-4" />
              <span className="text-xs font-medium">{t("characters.creationHelper.preview")}</span>
            </button>
          ) : null
        }
      />

      {/* Messages Container */}
      <main className="flex-1 overflow-y-auto px-4 pt-[calc(72px+env(safe-area-inset-top))] pb-32">
        <div className="mx-auto max-w-2xl space-y-4 py-4">
          <div className="flex justify-center">
            <div className="rounded-full border border-fg/10 bg-fg/5 px-3 py-1 text-[10px] uppercase tracking-wider text-fg/50">
              {t("characters.creationHelper.modeBadge", { goal: goalLabel })}
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
                {t("characters.creationHelper.creatorTitle", { goal: goalLabel })}
              </h2>
              <p className="text-fg/60 text-sm max-w-xs">
                {activeGoal === "persona"
                  ? t("characters.creationHelper.welcomePersona")
                  : t("characters.creationHelper.welcomeCharacter")}
              </p>
              <div className="mt-4 flex items-center gap-1">
                <Loader2 className="h-4 w-4 text-fg/40 animate-spin" />
                <span className="text-xs text-fg/40">{t("characters.creationHelper.starting")}</span>
              </div>
            </motion.div>
          )}

          {/* Messages */}
          <AnimatePresence initial={false}>
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
              const isLiveBubble =
                message.id === "__streaming__" || message.id === "__placeholder__";
              return (
                <motion.div
                  key={message.id}
                  initial={isLiveBubble ? { opacity: 0 } : false}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
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
                            <div className="h-36 w-36 rounded-lg border border-fg/10 bg-fg/5 flex items-center justify-center">
                              <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
                            </div>
                          </>
                        ) : imageEntry.status === "error" ? (
                          <>
                            <div className="h-36 w-36 rounded-lg border border-danger/30 bg-danger/10 flex items-center justify-center">
                              <span className="text-xs text-danger">{t("characters.creationHelper.generationFailed")}</span>
                            </div>
                          </>
                        ) : imageEntry.imageId ? (
                          <button
                            type="button"
                            onClick={() =>
                              setImagePreview({
                                id: imageEntry.imageId as string,
                                label: t("characters.creationHelper.generatedImage"),
                              })
                            }
                            className="group flex flex-col items-center gap-2"
                          >
                            <GeneratedImagePreview
                              sessionId={session?.id ?? ""}
                              imageId={imageEntry.imageId}
                              label={t("characters.creationHelper.imageReady")}
                              size="sm"
                              className="transition-transform group-hover:scale-[1.01]"
                            />
                          </button>
                        ) : (
                          <span className="text-xs text-fg/40">{t("characters.creationHelper.imageUnavailable")}</span>
                        )}
                      </div>
                    ) : (
                      <>
                        {message.blocks && message.blocks.length > 0 ? (
                          <div className="space-y-2">
                            {(() => {
                              const elements: React.ReactNode[] = [];
                              const blocks = message.blocks!;
                              let i = 0;
                              while (i < blocks.length) {
                                const b = blocks[i];
                                if (b.kind === "text") {
                                  elements.push(
                                    <MarkdownRenderer
                                      key={`b-${i}`}
                                      content={b.content}
                                      className={cn(
                                        "text-sm leading-relaxed",
                                        message.role === "user" ? "text-fg" : "text-fg/90",
                                      )}
                                    />,
                                  );
                                  i++;
                                  continue;
                                }
                                const pillRow: React.ReactNode[] = [];
                                while (i < blocks.length && blocks[i].kind === "tool") {
                                  const tb = blocks[i] as { kind: "tool"; toolCallId: string };
                                  const call = (message.toolCalls || []).find(
                                    (c) => c.id === tb.toolCallId,
                                  );
                                  const result = (message.toolResults || []).find(
                                    (r) => r.toolCallId === tb.toolCallId,
                                  );
                                  i++;
                                  if (!call) continue;
                                  const displayName = getToolDisplayName(
                                    call.name || t("characters.creationHelper.unknownTool"),
                                  );
                                  const isPending = !result;
                                  pillRow.push(
                                    <button
                                      key={`p-${i}-${call.id}`}
                                      onClick={() => {
                                        if (!result) return;
                                        setSelectedTool({ call, result });
                                        setShowToolDetail(true);
                                      }}
                                      disabled={isPending}
                                      className={cn(
                                        "inline-flex items-center gap-1.5 text-[11px] rounded-full px-2 py-0.5 transition-colors",
                                        isPending
                                          ? "bg-fg/5 text-fg/60 border border-fg/10"
                                          : result.success
                                            ? "bg-accent/10 text-accent/80 hover:bg-accent/20"
                                            : "bg-danger/10 text-danger hover:bg-danger/20",
                                      )}
                                    >
                                      {isPending ? (
                                        <Loader2 className="h-3 w-3 shrink-0 animate-spin" />
                                      ) : result.success ? (
                                        <Check className="h-3 w-3 shrink-0" />
                                      ) : (
                                        <span className="h-3 w-3 shrink-0 leading-none">✗</span>
                                      )}
                                      <span className="truncate">{displayName}</span>
                                    </button>,
                                  );
                                }
                                if (pillRow.length > 0) {
                                  elements.push(
                                    <div
                                      key={`pr-${i}`}
                                      className="flex flex-wrap gap-1.5"
                                    >
                                      {pillRow}
                                    </div>,
                                  );
                                  const rowEndIndex = i;
                                  const rowStartIndex = (() => {
                                    let s = i - 1;
                                    while (s >= 0 && blocks[s].kind === "tool") s--;
                                    return s + 1;
                                  })();
                                  const rowImages: { id: string; label: string }[] = [];
                                  for (let j = rowStartIndex; j < rowEndIndex; j++) {
                                    const tb = blocks[j];
                                    if (tb.kind !== "tool") continue;
                                    const call = (message.toolCalls || []).find(
                                      (c) => c.id === tb.toolCallId,
                                    );
                                    if (!call || !isImageGenCall(call.name)) continue;
                                    const result = (message.toolResults || []).find(
                                      (r) => r.toolCallId === tb.toolCallId,
                                    );
                                    const resObj = result?.result as
                                      | { image_id?: string; imageId?: string }
                                      | undefined;
                                    const imgId = resObj?.image_id ?? resObj?.imageId;
                                    if (imgId) {
                                      rowImages.push({
                                        id: imgId,
                                        label: getToolDisplayName(call.name),
                                      });
                                    }
                                  }
                                  if (rowImages.length > 0 && session?.id) {
                                    elements.push(
                                      <div
                                        key={`pi-${i}`}
                                        className="flex flex-wrap gap-1.5 -mt-0.5"
                                      >
                                        {rowImages.map((img) => (
                                          <button
                                            key={img.id}
                                            type="button"
                                            onClick={() =>
                                              setImagePreview({ id: img.id, label: img.label })
                                            }
                                            className="rounded-lg overflow-hidden border border-fg/10 hover:border-fg/25 transition-colors"
                                            aria-label={img.label}
                                          >
                                            <GeneratedImagePreview
                                              sessionId={session.id}
                                              imageId={img.id}
                                              label={img.label}
                                              size="xs"
                                            />
                                          </button>
                                        ))}
                                      </div>,
                                    );
                                  }
                                }
                              }
                              if (message.id === "__streaming__") {
                                elements.push(
                                  <TypingIndicator key="trailing-dots" />,
                                );
                              }
                              return elements;
                            })()}
                          </div>
                        ) : (
                          (() => {
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

                            if (
                              (message.id === "__streaming__" || message.id === "__placeholder__") &&
                              !displayText.trim()
                            ) {
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
                          })()
                        )}

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
                  <span>{t("characters.creationHelper.regenerateResponse")}</span>
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
          previewDismissedRef.current = true;
        }}
        title={previewTitle}
      >
        {activeGoal === "character" && session?.draft && (
          <div className="space-y-4">
            <CharacterPreviewCard
              draft={session.draft}
              sessionId={session.id}
              targetCharacterId={
                session.creationMode === "edit" && session.targetType === "character"
                  ? (session.targetId ?? undefined)
                  : undefined
              }
            />

            <MenuSection>
              {session.creationMode === "edit" && session.targetType === "character" ? (
                <MenuButton
                  icon={Check}
                  title={t("characters.creationHelper.saveCharacterChanges")}
                  description={t("characters.creationHelper.saveCharacterChangesDesc")}
                  color="from-accent to-accent/80"
                  onClick={handleUseCharacter}
                />
              ) : (
                <>
                  <MenuButton
                    icon={Check}
                    title={t("characters.creationHelper.saveAndChat")}
                    description={t("characters.creationHelper.saveAndChatDesc")}
                    color="from-accent to-accent/80"
                    onClick={handleSaveAndChat}
                  />
                  <MenuButton
                    icon={PenLine}
                    title={t("characters.creationHelper.openInEditor")}
                    description={t("characters.creationHelper.openInEditorDesc")}
                    color="from-warning to-warning/80"
                    onClick={handleEditManually}
                  />
                </>
              )}
              <MenuButton
                icon={RefreshCw}
                title={t("characters.creationHelper.keepEditingHere")}
                description={t("characters.creationHelper.keepEditingHereDesc")}
                color="from-info to-info/80"
                onClick={() => {
                  setShowPreview(false);
                  setShowConfirmation(false);
                }}
              />
            </MenuSection>
          </div>
        )}

        {activeGoal === "persona" && (
          <div className="space-y-4">
            <PersonaPreviewCard
              persona={previewPersona}
              sessionId={session?.id}
              targetPersonaId={
                session?.creationMode === "edit" && session?.targetType === "persona"
                  ? (session.targetId ?? undefined)
                  : undefined
              }
            />

            <MenuSection>
              <MenuButton
                icon={User}
                title={
                  session?.creationMode === "edit" && session?.targetType === "persona"
                    ? t("characters.creationHelper.savePersonaChanges")
                    : t("characters.creationHelper.usePersona")
                }
                description={
                  session?.creationMode === "edit" && session?.targetType === "persona"
                    ? t("characters.creationHelper.savePersonaChangesDesc")
                    : t("characters.creationHelper.usePersonaDesc")
                }
                color="from-accent to-accent/80"
                onClick={handleOpenPersona}
                disabled={
                  session?.creationMode === "edit" && session?.targetType === "persona"
                    ? !session?.targetId
                    : !(session?.draft?.name?.trim() || session?.draft?.description?.trim() ||
                        session?.draft?.definition?.trim())
                }
              />
              <MenuButton
                icon={RefreshCw}
                title={t("characters.creationHelper.keepEditing")}
                description={t("characters.creationHelper.keepEditingDesc")}
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
        title={
          selectedTool
            ? getToolDisplayName(selectedTool.call.name)
            : t("characters.creationHelper.toolUsageDetails")
        }
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
                <h3 className={cn(typography.h2.size, typography.h2.weight, "text-fg text-base")}>
                  {selectedTool.result.success
                    ? t("characters.creationHelper.executionSuccessful")
                    : t("characters.creationHelper.executionFailed")}
                </h3>
                <p className="text-fg/40 text-[10px] uppercase tracking-wider font-bold">
                  {t("characters.creationHelper.toolLabel", { tool: selectedTool.call.name })}
                </p>
              </div>
            </div>

            <div className="space-y-4">
              <div className="space-y-2">
                <h4 className="text-[10px] font-bold text-fg/30 uppercase tracking-wider px-1">
                  {t("characters.creationHelper.modelInputArguments")}
                </h4>
                <div className="bg-surface-el/40 rounded-xl p-3 border border-fg/5 overflow-x-auto">
                  <pre className="text-xs text-info font-mono leading-relaxed">
                    {JSON.stringify(selectedTool.call.arguments, null, 2)}
                  </pre>
                </div>
              </div>

              <div className="space-y-2">
                <h4 className="text-[10px] font-bold text-fg/30 uppercase tracking-wider px-1">
                  {t("characters.creationHelper.toolOutputResult")}
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
        title={imagePreview?.label ?? t("characters.creationHelper.generatedImageTitle")}
      >
        {imagePreview && session?.id && (
          <div className="flex flex-col items-center gap-2 py-6">
            <GeneratedImagePreview
              sessionId={session.id}
              imageId={imagePreview.id}
              label={imagePreview.label}
              size="lg"
            />
            <button
              type="button"
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(imagePreview.id);
                  toast.success(t("characters.creationHelper.imageIdCopied"));
                } catch {
                  toast.error(t("characters.creationHelper.copyFailed"));
                }
              }}
              className="rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm text-white/90 text-center hover:bg-white/15 transition-colors cursor-pointer"
            >
              <span className="text-white/60 select-none">{t("characters.creationHelper.idLabel")}</span>{" "}
              <span className="select-all">{imagePreview.id}</span>
            </button>
          </div>
        )}
      </BottomMenu>
    </div>
  );
}
