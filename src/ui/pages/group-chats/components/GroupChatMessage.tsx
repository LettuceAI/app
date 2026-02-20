import { motion, type PanInfo, AnimatePresence } from "framer-motion";
import React, { useCallback, useMemo, useState, useRef } from "react";
import { RefreshCw, Pin, User, Bot, ChevronDown, Volume2, Loader2, Square } from "lucide-react";
import { MarkdownRenderer } from "../../chats/components/MarkdownRenderer";
import type {
  GroupMessage,
  GroupMessageVariant,
  Character,
  Persona,
} from "../../../../core/storage/schemas";
import type { ThemeColors } from "../../../../core/utils/imageAnalysis";
import type { ChatAppearanceSettings } from "../../../../core/storage/schemas";
import { radius, typography, interactive, cn } from "../../../design-tokens";
import { AvatarImage } from "../../../components/AvatarImage";
import { useAvatar } from "../../../hooks/useAvatar";
import { useSessionAttachments } from "../../../hooks/useSessionAttachment";
import { replaceCharacterPlaceholders } from "../utils/replaceCharacterPlaceholders";

export interface VariantState {
  total: number;
  selectedIndex: number;
  variants?: GroupMessageVariant[];
}

// CSS class mappings for chat appearance settings
const FONT_SIZE_MAP = { small: "text-xs", medium: "text-sm", large: "text-base", xlarge: "text-lg" } as const;
const LINE_SPACING_MAP = { tight: "leading-snug", normal: "leading-normal", relaxed: "leading-relaxed" } as const;
const BUBBLE_RADIUS_MAP = { sharp: "rounded-md", rounded: "rounded-lg", pill: "rounded-2xl" } as const;
const BUBBLE_MAX_WIDTH_MAP = { compact: "max-w-[70%]", normal: "max-w-[82%]", wide: "max-w-[92%]" } as const;
const BUBBLE_PADDING_MAP = { compact: "px-3 py-1.5", normal: "px-4 py-2.5", spacious: "px-5 py-3.5" } as const;
const BUBBLE_BLUR_MAP = { none: "", light: "backdrop-blur-sm", medium: "backdrop-blur-md", heavy: "backdrop-blur-lg" } as const;
const AVATAR_SHAPE_MAP = { circle: "rounded-full", rounded: "rounded-lg", hidden: "" } as const;
const AVATAR_SIZE_MAP = { small: "h-6 w-6", medium: "h-8 w-8", large: "h-10 w-10" } as const;
const AVATAR_ICON_SIZE_MAP = { small: 12, medium: 16, large: 20 } as const;

export interface GroupChatMessageProps {
  message: GroupMessage;
  index: number;
  messagesLength: number;
  heldMessageId: string | null;
  regeneratingMessageId: string | null;
  sending: boolean;
  character?: Character;
  persona?: Persona | null;
  characters?: Character[];
  theme: ThemeColors;
  chatAppearance?: ChatAppearanceSettings;
  getVariantState: (message: GroupMessage) => VariantState;
  handleVariantDrag: (messageId: string, offsetX: number) => void;
  handleRegenerate: (message: GroupMessage) => Promise<void>;
  onLongPress: (message: GroupMessage) => void;
  displayContent?: string;
  onImageClick?: (src: string, alt: string) => void;
  audioStatus?: "loading" | "playing";
  onPlayAudio?: (message: GroupMessage, text: string) => Promise<void>;
  onStopAudio?: (message: GroupMessage) => void;
  onCancelAudio?: (message: GroupMessage) => void;
  reasoning?: string;
}

// Avatar component for user/assistant
const MessageAvatar = React.memo(function MessageAvatar({
  role,
  character,
  persona,
  avatarShape = "circle",
  avatarSize = "medium",
}: {
  role: "user" | "assistant" | "scene";
  character?: Character;
  persona?: Persona | null;
  avatarShape?: "circle" | "rounded" | "hidden";
  avatarSize?: "small" | "medium" | "large";
}) {
  const characterAvatar = useAvatar(
    "character",
    character?.id ?? "",
    character?.avatarPath,
    "round",
  );
  const personaAvatar = useAvatar("persona", persona?.id ?? "", persona?.avatarPath, "round");

  if (avatarShape === "hidden") return null;

  const sizeClass = AVATAR_SIZE_MAP[avatarSize];
  const shapeClass = AVATAR_SHAPE_MAP[avatarShape];
  const iconSize = AVATAR_ICON_SIZE_MAP[avatarSize];

  if (role === "user") {
    return (
      <div className={cn("relative flex shrink-0 items-center justify-center overflow-hidden border border-fg/10 bg-linear-to-br from-white/5 to-white/10", sizeClass, shapeClass)}>
        {personaAvatar ? (
          <AvatarImage src={personaAvatar} alt="User" crop={persona?.avatarCrop} applyCrop />
        ) : (
          <User size={iconSize} className="text-fg/60" />
        )}
      </div>
    );
  }

  if (role === "assistant" || role === "scene") {
    return (
      <div className={cn("relative flex shrink-0 items-center justify-center overflow-hidden border border-fg/10 bg-linear-to-br from-white/5 to-white/10", sizeClass, shapeClass)}>
        {characterAvatar ? (
          <AvatarImage
            src={characterAvatar}
            alt={character?.name || "Assistant"}
            crop={character?.avatarCrop}
            applyCrop
          />
        ) : (
          <Bot size={iconSize} className="text-fg/60" />
        )}
      </div>
    );
  }

  return null;
});

// Memoized action buttons component
const MessageActions = React.memo(function MessageActions({
  disabled,
  isRegenerating,
  onRegenerate,
}: {
  disabled: boolean;
  isRegenerating: boolean;
  onRegenerate: () => void;
}) {
  return (
    <motion.div
      className="absolute -bottom-4 right-0 flex items-center gap-2"
      initial={{ opacity: 0, scale: 0.9 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{
        type: "tween",
        duration: 0.15,
        ease: [0.25, 0.46, 0.45, 0.94],
        delay: 0.1,
      }}
    >
      <button
        type="button"
        onClick={onRegenerate}
        disabled={disabled}
        className={cn(
          "flex items-center px-[0.6em] py-[0.3em] justify-center",
          radius.full,
          "border border-fg/15 bg-fg/10 text-fg",
          interactive.transition.fast,
          "hover:border-fg/30 hover:bg-fg/20 hover:scale-105",
          interactive.active.scale,
          "disabled:cursor-not-allowed disabled:opacity-80 disabled:hover:scale-100",
        )}
        aria-label="Regenerate response"
        style={{ willChange: "transform" }}
      >
        {isRegenerating ? (
          <RefreshCw size={14} className="animate-spin rounded-full" />
        ) : (
          <RefreshCw size={14} />
        )}
      </button>
    </motion.div>
  );
});

const ThinkingSection = React.memo(function ThinkingSection({
  reasoning,
  isStreaming,
}: {
  reasoning: string;
  isStreaming: boolean;
}) {
  const [isExpanded, setIsExpanded] = useState(false);

  const THINKING_TEXTS = [
    "Thinking really hard…",
    "Consulting the lettuce council…",
    "Stealing thoughts from the void…",
    "Warming up the brain cells…",
    "Loading forbidden knowledge…",
    "Overthinking (as usual)…",
    "Pretending to be smart…",
    "Crunching imaginary numbers…",
    "Arguing with myself…",
    "Asking the universe nicely…",
  ];

  const thinkingText = React.useMemo(() => {
    if (!isStreaming) return null;
    return THINKING_TEXTS[Math.floor(Math.random() * THINKING_TEXTS.length)];
  }, [isStreaming]);

  if (!reasoning || reasoning.trim().length === 0) {
    return null;
  }

  return (
    <div className="mb-2">
      <button
        type="button"
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          "flex items-center gap-2 py-1 text-left text-xs transition-colors",
          isStreaming ? "text-fg/60 hover:text-fg/80" : "text-fg/40 hover:text-fg/60",
        )}
      >
        <motion.div
          animate={{ rotate: isExpanded ? 90 : 0 }}
          transition={{ duration: 0.15 }}
          className="opacity-60"
        >
          <ChevronDown size={12} />
        </motion.div>
        <span className="flex items-center gap-1.5">
          {isStreaming && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-fg/60" />}
          <span className="font-medium">{isStreaming ? thinkingText : "Thought process"}</span>
        </span>
      </button>

      <AnimatePresence mode="wait">
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div
              className={cn(
                "mt-1 pl-5 border-l border-fg/10",
                "text-xs text-fg/40 italic leading-relaxed",
                "max-h-40 overflow-y-auto",
              )}
            >
              <MarkdownRenderer
                content={reasoning}
                className="text-xs text-fg/40 **:text-fg/40"
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

function GroupChatMessageInner({
  message,
  index,
  messagesLength,
  heldMessageId,
  regeneratingMessageId,
  sending,
  character,
  persona,
  characters = [],
  theme,
  chatAppearance,
  getVariantState,
  handleVariantDrag,
  handleRegenerate,
  onLongPress,
  displayContent,
  onImageClick,
  audioStatus,
  onPlayAudio,
  onStopAudio,
  onCancelAudio,
  reasoning,
}: GroupChatMessageProps) {
  const longPressTimer = useRef<number | null>(null);
  const isLongPress = useRef(false);

  // Memoize all computed values
  const computed = useMemo(() => {
    const isAssistant = message.role === "assistant";
    const isScene = message.role === "scene";
    const isUser = message.role === "user";
    const isPlaceholder = message.id.startsWith("temp-") || message.id.startsWith("placeholder");
    const actionable = (isAssistant || isUser || isScene) && !isPlaceholder;
    const isLatestAssistant = isAssistant && actionable && index === messagesLength - 1;
    const variantState = getVariantState(message);
    const totalVariants = variantState.total || (isAssistant || isScene ? 1 : 0);
    const selectedVariantIndex =
      variantState.selectedIndex >= 0
        ? variantState.selectedIndex
        : totalVariants > 0
          ? totalVariants - 1
          : -1;

    const enableSwipe = isLatestAssistant && (variantState.variants?.length ?? 0) > 1;

    const showTypingIndicator = isAssistant && isPlaceholder && message.content.trim().length === 0;
    const showRegenerateButton = isLatestAssistant && !isScene;
    const shouldAnimate = !isPlaceholder;

    return {
      isAssistant,
      isScene,
      isUser,
      isPlaceholder,
      isLatestAssistant,
      totalVariants,
      selectedVariantIndex,
      enableSwipe,
      showTypingIndicator,
      showRegenerateButton,
      shouldAnimate,
    };
  }, [message.role, message.id, message.content, index, messagesLength, getVariantState]);

  // Replace character placeholders in content for scene messages
  const processedContent = useMemo(() => {
    const content = displayContent ?? message.content;
    if (computed.isScene && characters.length > 0) {
      return replaceCharacterPlaceholders(content, characters);
    }
    return content;
  }, [displayContent, message.content, computed.isScene, characters]);

  const playText = processedContent;
  const voiceConfig = character?.voiceConfig;
  const hasVoiceAssignment =
    voiceConfig?.source === "user"
      ? !!voiceConfig.userVoiceId
      : voiceConfig?.source === "provider"
        ? !!voiceConfig.providerId && !!voiceConfig.voiceId
        : false;
  const canPlayAudio =
    (computed.isAssistant || computed.isScene) &&
    !computed.isPlaceholder &&
    hasVoiceAssignment &&
    playText.trim().length > 0;
  const isAudioLoading = audioStatus === "loading";
  const isAudioPlaying = audioStatus === "playing";

  const handlePlayAudio = useCallback(
    async (event: React.MouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      if (!canPlayAudio) return;
      if (isAudioLoading) {
        onCancelAudio?.(message);
        return;
      }
      if (isAudioPlaying) {
        onStopAudio?.(message);
        return;
      }
      if (!onPlayAudio) return;
      try {
        await onPlayAudio(message, playText);
      } catch (error) {
        console.error("Failed to play message audio:", error);
      }
    },
    [
      canPlayAudio,
      isAudioLoading,
      isAudioPlaying,
      message,
      onCancelAudio,
      onPlayAudio,
      onStopAudio,
      playText,
    ],
  );

  const dragProps = useMemo(
    () =>
      computed.enableSwipe
        ? {
            drag: "x" as const,
            dragConstraints: { left: -140, right: 140 },
            dragElastic: 0.08,
            dragMomentum: false,
            dragSnapToOrigin: true,
            dragTransition: { bounceStiffness: 600, bounceDamping: 40 },
            onDragEnd: (_: unknown, info: PanInfo) =>
              void handleVariantDrag(message.id, info.offset.x),
            whileDrag: { scale: 0.98 },
            style: { willChange: "transform", transform: "translate3d(0,0,0)" },
          }
        : {},
    [computed.enableSwipe, handleVariantDrag, message.id],
  );

  const animTransition = useMemo(
    () =>
      computed.shouldAnimate
        ? { type: "tween" as const, duration: 0.2, ease: [0.25, 0.46, 0.45, 0.94] as const }
        : { duration: 0 },
    [computed.shouldAnimate],
  );

  // Long press handlers
  const handlePointerDown = useCallback(() => {
    isLongPress.current = false;
    longPressTimer.current = window.setTimeout(() => {
      isLongPress.current = true;
      onLongPress(message);
    }, 500);
  }, [message, onLongPress]);

  const handlePointerUp = useCallback(() => {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }, []);

  const handlePointerLeave = useCallback(() => {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }, []);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      onLongPress(message);
    },
    [message, onLongPress],
  );

  // Event handlers for long press
  const eventHandlers = useMemo(
    () => ({
      onPointerDown: handlePointerDown,
      onPointerUp: handlePointerUp,
      onPointerLeave: handlePointerLeave,
      onContextMenu: handleContextMenu,
    }),
    [handlePointerDown, handlePointerUp, handlePointerLeave, handleContextMenu],
  );

  // Load attachments with lazy loading support for persisted images
  const loadedAttachments = useSessionAttachments(message.attachments);

  return (
    <div
      className={cn(
        "relative flex gap-2",
        message.role === "user" ? "justify-end" : "justify-start",
      )}
    >
      {/* Avatar for assistant/scene messages (left side) */}
      {(message.role === "assistant" || message.role === "scene") && chatAppearance?.avatarShape !== "hidden" && (
        <div className="flex shrink-0 flex-col items-center gap-1">
          <MessageAvatar role={message.role} character={character} persona={persona} avatarShape={chatAppearance?.avatarShape} avatarSize={chatAppearance?.avatarSize} />
          {canPlayAudio && (
            <button
              type="button"
              onClick={handlePlayAudio}
              className={cn(
                "audio-btn flex h-6 w-6 items-center justify-center rounded-full",
                "border border-fg/40 bg-fg/10 text-fg shadow-sm",
                "transition hover:bg-fg/20 active:scale-95",
                "disabled:cursor-not-allowed disabled:opacity-70",
              )}
              aria-label={
                isAudioLoading
                  ? "Cancel audio generation"
                  : isAudioPlaying
                    ? "Stop audio"
                    : "Play message audio"
              }
              aria-pressed={isAudioPlaying}
              aria-busy={isAudioLoading}
              title={
                isAudioLoading
                  ? "Cancel audio generation"
                  : isAudioPlaying
                    ? "Stop audio"
                    : "Play audio"
              }
            >
              {isAudioLoading ? (
                <Loader2 size={16} className="animate-spin" />
              ) : isAudioPlaying ? (
                <Square size={14} />
              ) : (
                <Volume2 size={16} />
              )}
            </button>
          )}
        </div>
      )}

      <motion.div
        initial={computed.shouldAnimate ? { opacity: 0, y: 4 } : false}
        animate={computed.shouldAnimate ? { opacity: 1, y: 0 } : { opacity: 1, y: 0 }}
        transition={animTransition}
        className={cn(
          chatAppearance ? BUBBLE_MAX_WIDTH_MAP[chatAppearance.bubbleMaxWidth] : "max-w-[82%]",
          chatAppearance ? BUBBLE_PADDING_MAP[chatAppearance.bubblePadding] : "px-4 py-2.5",
          chatAppearance ? LINE_SPACING_MAP[chatAppearance.lineSpacing] : "leading-relaxed",
          chatAppearance ? BUBBLE_RADIUS_MAP[chatAppearance.bubbleRadius] : radius.lg,
          chatAppearance ? FONT_SIZE_MAP[chatAppearance.fontSize] : typography.body.size,
          chatAppearance ? BUBBLE_BLUR_MAP[chatAppearance.bubbleBlur] : "",
          message.role === "user"
            ? cn(
                "ml-auto",
                chatAppearance?.bubbleStyle === "minimal"
                  ? `${theme.userText}`
                  : `${theme.userBg} ${theme.userText} border ${theme.userBorder}`,
                heldMessageId === message.id && "ring-2 ring-accent/50",
              )
            : cn(
                chatAppearance?.bubbleStyle === "minimal"
                  ? `${theme.assistantText}`
                  : `border ${theme.assistantBg} ${theme.assistantText}`,
                chatAppearance?.bubbleStyle !== "minimal" && (heldMessageId === message.id ? "border-fg/30" : theme.assistantBorder),
              ),
        )}
        {...eventHandlers}
        {...dragProps}
      >
        {/* Pin indicator */}
        {message.isPinned && (
          <motion.div
            className="absolute -top-2 -right-2 z-10 flex h-5 w-5 items-center justify-center rounded-full border border-info/40 bg-info/20 shadow-lg"
            initial={{ scale: 0, rotate: -45 }}
            animate={{ scale: 1, rotate: 0 }}
            transition={{ type: "spring", stiffness: 300, damping: 20 }}
          >
            <Pin size={12} className="text-info" />
          </motion.div>
        )}

        {/* Thinking/Reasoning section - shown even during typing indicator */}
        {computed.isAssistant && reasoning && (
          <ThinkingSection reasoning={reasoning} isStreaming={computed.isPlaceholder && sending} />
        )}

        {/* Show typing indicator only if no reasoning (reasoning section has its own indicator) */}
        {computed.showTypingIndicator && !reasoning ? (
          <TypingIndicator />
        ) : computed.showTypingIndicator ? null : (
          <>
            {/* Display attachments if present (with lazy loading support) */}
            {loadedAttachments.length > 0 && (
              <div className="mb-2 flex flex-wrap gap-2">
                {loadedAttachments.map((attachment) => (
                  <div
                    key={attachment.id}
                    className={cn(
                      radius.md,
                      "overflow-hidden border border-fg/15",
                      attachment.data &&
                        onImageClick &&
                        "cursor-pointer hover:border-fg/30 transition-colors",
                    )}
                    onClick={() =>
                      attachment.data &&
                      onImageClick?.(attachment.data, attachment.filename || "Attached image")
                    }
                  >
                    {attachment.data ? (
                      <img
                        src={attachment.data}
                        alt={attachment.filename || "Attached image"}
                        className="max-h-48 max-w-full object-contain"
                        style={{
                          maxWidth:
                            attachment.width && attachment.width > 300
                              ? 300
                              : attachment.width || 300,
                        }}
                      />
                    ) : (
                      // Loading placeholder
                      <div
                        className="flex items-center justify-center bg-fg/5"
                        style={{
                          width: Math.min(attachment.width || 150, 300),
                          height: Math.min(attachment.height || 100, 192),
                        }}
                      >
                        <div className="h-5 w-5 animate-spin rounded-full border-2 border-fg/20 border-t-white/60" />
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            <MarkdownRenderer
              key={message.id + ":" + computed.selectedVariantIndex}
              content={processedContent}
              className="text-inherit select-none"
            />
          </>
        )}

        {(computed.isAssistant || computed.isScene) && computed.totalVariants > 1 && (
          <motion.div
            className={cn(
              "mt-2.5 flex items-center justify-between pr-2",
              typography.caption.size,
              typography.caption.weight,
              "uppercase tracking-wider text-fg/40",
            )}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.2, delay: 0.15 }}
          >
            <span className="text-fg">
              Variant {computed.selectedVariantIndex >= 0 ? computed.selectedVariantIndex + 1 : 1}
              {computed.totalVariants > 0 ? ` / ${computed.totalVariants}` : ""}
            </span>
            {regeneratingMessageId === message.id && (
              <motion.span
                className="flex items-center gap-1.5 text-accent/80"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ duration: 0.15 }}
              >
                <span className="h-3 w-3 animate-spin rounded-full border-2 border-accent/30 border-t-accent/80" />
                Regenerating
              </motion.span>
            )}
          </motion.div>
        )}
      </motion.div>

      {/* Avatar for user messages (right side) */}
      {message.role === "user" && chatAppearance?.avatarShape !== "hidden" && (
        <MessageAvatar role={message.role} character={character} persona={persona} avatarShape={chatAppearance?.avatarShape} avatarSize={chatAppearance?.avatarSize} />
      )}

      {computed.showRegenerateButton && (
        <MessageActions
          disabled={regeneratingMessageId === message.id || sending}
          isRegenerating={regeneratingMessageId === message.id}
          onRegenerate={() => void handleRegenerate(message)}
        />
      )}
    </div>
  );
}

export const GroupChatMessage = React.memo(GroupChatMessageInner, (prev, next) => {
  const a = prev.message;
  const b = next.message;
  const aAttachments = a.attachments ?? [];
  const bAttachments = b.attachments ?? [];
  return (
    a.id === b.id &&
    a.role === b.role &&
    a.content === b.content &&
    a.selectedVariantId === b.selectedVariantId &&
    (a.variants?.length ?? 0) === (b.variants?.length ?? 0) &&
    aAttachments.length === bAttachments.length &&
    aAttachments.every((att, idx) => {
      const other = bAttachments[idx];
      return (
        att.id === other?.id &&
        att.data === other?.data &&
        att.storagePath === other?.storagePath &&
        att.mimeType === other?.mimeType &&
        att.width === other?.width &&
        att.height === other?.height
      );
    }) &&
    prev.index === next.index &&
    prev.messagesLength === next.messagesLength &&
    prev.heldMessageId === next.heldMessageId &&
    prev.regeneratingMessageId === next.regeneratingMessageId &&
    prev.sending === next.sending &&
    prev.theme === next.theme &&
    prev.chatAppearance === next.chatAppearance &&
    prev.character?.id === next.character?.id &&
    prev.character?.avatarPath === next.character?.avatarPath &&
    prev.character?.avatarCrop?.x === next.character?.avatarCrop?.x &&
    prev.character?.avatarCrop?.y === next.character?.avatarCrop?.y &&
    prev.character?.avatarCrop?.scale === next.character?.avatarCrop?.scale &&
    (() => {
      const aVoice = prev.character?.voiceConfig;
      const bVoice = next.character?.voiceConfig;
      if (!aVoice && !bVoice) return true;
      if (!aVoice || !bVoice) return false;
      return (
        aVoice.source === bVoice.source &&
        aVoice.userVoiceId === bVoice.userVoiceId &&
        aVoice.providerId === bVoice.providerId &&
        aVoice.voiceId === bVoice.voiceId &&
        aVoice.modelId === bVoice.modelId &&
        aVoice.voiceName === bVoice.voiceName
      );
    })() &&
    prev.persona?.id === next.persona?.id &&
    prev.persona?.avatarPath === next.persona?.avatarPath &&
    prev.persona?.avatarCrop?.x === next.persona?.avatarCrop?.x &&
    prev.persona?.avatarCrop?.y === next.persona?.avatarCrop?.y &&
    prev.persona?.avatarCrop?.scale === next.persona?.avatarCrop?.scale &&
    prev.audioStatus === next.audioStatus &&
    a.reasoning === b.reasoning &&
    prev.reasoning === next.reasoning &&
    prev.onPlayAudio === next.onPlayAudio &&
    prev.onStopAudio === next.onStopAudio &&
    prev.onCancelAudio === next.onCancelAudio
  );
});

function TypingIndicator() {
  return (
    <div className="flex items-center gap-1" aria-label="Assistant is typing" aria-live="polite">
      <span className="typing-dot" />
      <span className="typing-dot" style={{ animationDelay: "0.2s" }} />
      <span className="typing-dot" style={{ animationDelay: "0.4s" }} />
    </div>
  );
}
