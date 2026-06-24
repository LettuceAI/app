import { motion, type PanInfo, AnimatePresence } from "framer-motion";
import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { RefreshCw, Pin, User, Bot, ChevronDown, Volume2, Loader2, Square } from "lucide-react";
import { MarkdownRenderer } from "./MarkdownRenderer";
import { SceneContent } from "./SceneContent";
import type { StoredMessage, Character, Persona } from "../../../../core/storage/schemas";
import { radius, typography, interactive, cn } from "../../../design-tokens";
import { BottomMenu } from "../../../components/BottomMenu";
import type { ThemeColors } from "../../../../core/utils/imageAnalysis";
import type { ChatAppearanceSettings } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { useAvatar } from "../../../hooks/useAvatar";
import { useSessionAttachments } from "../../../hooks/useSessionAttachment";
import { AudioAttachmentPlayer } from "./AudioAttachmentPlayer";
import { useI18n, type TranslationKey, type TranslateParams } from "../../../../core/i18n/context";
import { replacePlaceholders } from "../../../../core/utils/placeholders";

interface VariantState {
  total: number;
  selectedIndex: number;
  variants?: Array<{ id: string; content: string; createdAt: number }>;
}

interface ChatMessageProps {
  message: StoredMessage;
  index: number;
  messagesLength: number;
  heldMessageId: string | null;
  regeneratingMessageId: string | null;
  sending: boolean;
  eventHandlers: Record<string, any>;
  getVariantState: (message: StoredMessage) => VariantState;
  handleVariantDrag: (messageId: string, offsetX: number) => void;
  handleRegenerate: (message: StoredMessage, options?: { guidance?: string }) => Promise<void>;
  isStartingSceneMessage: boolean;
  theme: ThemeColors;
  chatAppearance?: ChatAppearanceSettings;
  character: Character | null;
  persona: Persona | null;
  displayContent?: string;
  onImageClick?: (src: string, alt: string) => void;
  audioStatus?: "loading" | "playing";
  onPlayAudio?: (message: StoredMessage, text: string) => Promise<void>;
  onStopAudio?: (message: StoredMessage) => void;
  onCancelAudio?: (message: StoredMessage) => void;
  reasoning?: string;
  swapPlaces?: boolean;
  modelName?: string;
  scenePromptStreaming?: boolean;
}

// CSS class mappings for chat appearance settings
const FONT_SIZE_MAP = {
  small: "text-xs",
  medium: "text-sm",
  large: "text-base",
  xlarge: "text-lg",
} as const;
const LINE_SPACING_MAP = {
  tight: "leading-snug",
  normal: "leading-normal",
  relaxed: "leading-relaxed",
} as const;
const BUBBLE_RADIUS_MAP = {
  sharp: "rounded-md",
  rounded: "rounded-lg",
  pill: "rounded-2xl",
} as const;
const BUBBLE_MAX_WIDTH_MAP = {
  compact: "max-w-[70%]",
  normal: "max-w-[82%]",
  wide: "max-w-[92%]",
} as const;
const BUBBLE_PADDING_MAP = {
  compact: "px-3 py-1.5",
  normal: "px-4 py-2.5",
  spacious: "px-5 py-3.5",
} as const;
const MESSAGE_INFO_SIZE_MAP = {
  small: "text-[10px]",
  medium: "text-[11.5px]",
  large: "text-[13px]",
} as const;
const AVATAR_SHAPE_MAP = { circle: "rounded-full", rounded: "rounded-lg", hidden: "" } as const;
const AVATAR_SIZE_MAP = { small: "h-6 w-6", medium: "h-8 w-8", large: "h-10 w-10" } as const;
const AVATAR_ICON_SIZE_MAP = { small: 12, medium: 16, large: 20 } as const;

function formatMessageTimestamp(
  ms: number,
  format: "relative" | "time" | "datetime",
  t: (key: TranslationKey, params?: TranslateParams) => string,
): string {
  const date = new Date(ms);
  const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (format === "time") return time;
  if (format === "datetime") {
    return `${date.toLocaleDateString()} ${time}`;
  }
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const dayMs = 86_400_000;
  if (ms >= startOfToday) return time;
  if (ms >= startOfToday - dayMs) return t("chats.message.yesterdayAt", { time });
  return `${date.toLocaleDateString()}, ${time}`;
}

function BubbleHeaderWrap({
  above,
  rightAligned,
  header,
  aboveBubble,
  footer,
  maxWidthClass,
  children,
}: {
  above: boolean;
  rightAligned: boolean;
  header: React.ReactNode;
  aboveBubble?: React.ReactNode;
  footer?: React.ReactNode;
  maxWidthClass: string;
  children: React.ReactNode;
}) {
  if (!above && !aboveBubble && !footer) return <>{children}</>;
  return (
    <div
      className={cn(
        "flex min-w-0 flex-col",
        maxWidthClass,
        rightAligned ? "items-end" : "items-start",
      )}
    >
      {above && header}
      {aboveBubble && <div className="mb-1 px-1">{aboveBubble}</div>}
      {children}
      {footer && <div className="mt-1 px-1">{footer}</div>}
    </div>
  );
}

// Avatar component for user/assistant
const MessageAvatar = React.memo(function MessageAvatar({
  role,
  character,
  persona,
  avatarShape = "circle",
  avatarSize = "medium",
}: {
  role: "user" | "assistant" | "scene" | "system";
  character: Character | null;
  persona: Persona | null;
  avatarShape?: "circle" | "rounded" | "hidden";
  avatarSize?: "small" | "medium" | "large";
}) {
  const { t } = useI18n();
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
      <div
        className={cn(
          "relative flex shrink-0 items-center justify-center overflow-hidden border border-white/10 bg-linear-to-br from-white/5 to-white/10",
          sizeClass,
          shapeClass,
        )}
      >
        {personaAvatar ? (
          <AvatarImage
            src={personaAvatar}
            alt={t("chats.message.userAvatarAlt")}
            crop={persona?.avatarCrop}
            applyCrop
          />
        ) : (
          <User size={iconSize} className="text-white/60" />
        )}
      </div>
    );
  }

  if (role === "assistant" || role === "scene") {
    return (
      <div
        className={cn(
          "relative flex shrink-0 items-center justify-center overflow-hidden border border-white/10 bg-linear-to-br from-white/5 to-white/10",
          sizeClass,
          shapeClass,
        )}
      >
        {characterAvatar ? (
          <AvatarImage
            src={characterAvatar}
            alt={t("chats.message.assistantAvatarAlt")}
            crop={character?.avatarCrop}
            applyCrop
          />
        ) : (
          <Bot size={iconSize} className="text-white/60" />
        )}
      </div>
    );
  }

  if (role === "system") {
    return (
      <div
        className={cn(
          "relative flex shrink-0 items-center justify-center overflow-hidden border border-white/10 bg-linear-to-br from-white/5 to-white/10",
          sizeClass,
          shapeClass,
        )}
      >
        <Bot size={iconSize} className="text-white/60" />
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
  onGuidedRegenerate,
}: {
  disabled: boolean;
  isRegenerating: boolean;
  onRegenerate: () => void;
  onGuidedRegenerate: (guidance: string) => void;
}) {
  const { t } = useI18n();
  const [guidedOpen, setGuidedOpen] = useState(false);
  const [guidance, setGuidance] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const longPressTimerRef = useRef<number | null>(null);
  const longPressTriggeredRef = useRef(false);

  const clearLongPressTimer = useCallback(() => {
    if (longPressTimerRef.current !== null) {
      window.clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  }, []);

  const openGuidedRegeneration = useCallback(() => {
    if (disabled) return;
    longPressTriggeredRef.current = true;
    setGuidedOpen(true);
  }, [disabled]);

  const resetGuidedRegeneration = useCallback(() => {
    setGuidedOpen(false);
    setGuidance("");
  }, []);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLButtonElement>) => {
      if (disabled || event.button !== 0) return;
      clearLongPressTimer();
      longPressTriggeredRef.current = false;
      longPressTimerRef.current = window.setTimeout(openGuidedRegeneration, 450);
    },
    [clearLongPressTimer, disabled, openGuidedRegeneration],
  );

  const handlePointerEnd = useCallback(() => {
    clearLongPressTimer();
  }, [clearLongPressTimer]);

  const handleClick = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      clearLongPressTimer();
      if (longPressTriggeredRef.current) {
        event.preventDefault();
        event.stopPropagation();
        window.setTimeout(() => {
          longPressTriggeredRef.current = false;
        }, 0);
        return;
      }
      onRegenerate();
    },
    [clearLongPressTimer, onRegenerate],
  );

  const handleContextMenu = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      if (disabled) return;
      event.preventDefault();
      event.stopPropagation();
      clearLongPressTimer();
      openGuidedRegeneration();
    },
    [clearLongPressTimer, disabled, openGuidedRegeneration],
  );

  const submitGuidedRegeneration = useCallback(() => {
    const trimmed = guidance.trim();
    if (!trimmed || disabled) return;
    resetGuidedRegeneration();
    onGuidedRegenerate(trimmed);
  }, [disabled, guidance, onGuidedRegenerate, resetGuidedRegeneration]);

  useEffect(() => {
    if (!guidedOpen) return;
    const id = window.setTimeout(() => textareaRef.current?.focus(), 150);
    return () => window.clearTimeout(id);
  }, [guidedOpen]);

  useEffect(() => clearLongPressTimer, [clearLongPressTimer]);

  return (
    <>
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
          data-tour-id="chat-regenerate"
          onPointerDown={handlePointerDown}
          onPointerUp={handlePointerEnd}
          onPointerLeave={handlePointerEnd}
          onPointerCancel={handlePointerEnd}
          onClick={handleClick}
          onContextMenu={handleContextMenu}
          disabled={disabled}
          className={cn(
            "flex items-center px-[0.6em] py-[0.3em] justify-center",
            radius.full,
            "border border-fg/15 bg-surface-el/80 text-fg/85",
            interactive.transition.fast,
            "hover:border-fg/30 hover:bg-fg/12 hover:text-fg hover:scale-105",
            interactive.active.scale,
            "disabled:cursor-not-allowed disabled:opacity-80 disabled:hover:scale-100",
          )}
          aria-label={t("chats.message.regenerateResponse")}
          style={{ willChange: "transform" }}
        >
          {isRegenerating ? (
            <RefreshCw size={14} className="animate-spin rounded-full" />
          ) : (
            <RefreshCw size={14} />
          )}
        </button>
      </motion.div>

      <BottomMenu
        isOpen={guidedOpen}
        onClose={resetGuidedRegeneration}
        title={t("chats.message.guidedRegenerationTitle")}
      >
        <form
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            submitGuidedRegeneration();
          }}
        >
          <div className="space-y-1">
            <label className="block text-sm font-medium text-fg/90" htmlFor="guided-regeneration">
              {t("chats.message.guidedRegenerationLabel")}
            </label>
            <p className="text-sm leading-5 text-fg/55">
              {t("chats.message.guidedRegenerationDescription")}
            </p>
          </div>
          <textarea
            id="guided-regeneration"
            ref={textareaRef}
            value={guidance}
            onChange={(event) => setGuidance(event.target.value)}
            placeholder={t("chats.message.guidedRegenerationPlaceholder")}
            rows={4}
            className={cn(
              "w-full resize-none border border-fg/12 bg-fg/4 px-3 py-3 text-sm text-fg outline-none",
              radius.lg,
              "placeholder:text-fg/35 focus:border-fg/25 focus:bg-fg/6 focus:ring-2 focus:ring-fg/10",
            )}
          />
          <div className="flex gap-3">
            <button
              type="button"
              onClick={resetGuidedRegeneration}
              className={cn(
                "flex-1 border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg",
                radius.lg,
                "transition hover:border-fg/20 hover:bg-fg/10",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              type="submit"
              disabled={!guidance.trim() || disabled}
              className={cn(
                "flex-1 border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-100",
                radius.lg,
                "transition hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-emerald-500/20",
              )}
            >
              {t("chats.message.guidedRegenerationSubmit")}
            </button>
          </div>
        </form>
      </BottomMenu>
    </>
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
  const { t } = useI18n();

  const THINKING_TEXT_KEYS = [
    "chats.message.thinkingMessages.thinkingReallyHard",
    "chats.message.thinkingMessages.lettuceCouncil",
    "chats.message.thinkingMessages.stealingThoughts",
    "chats.message.thinkingMessages.warmingBrainCells",
    "chats.message.thinkingMessages.forbiddenKnowledge",
    "chats.message.thinkingMessages.overthinking",
    "chats.message.thinkingMessages.pretendingToBeSmart",
    "chats.message.thinkingMessages.crunchingNumbers",
    "chats.message.thinkingMessages.arguingWithMyself",
    "chats.message.thinkingMessages.askingUniverse",
  ] as const;

  const thinkingText = React.useMemo(() => {
    if (!isStreaming) return null;
    const key = THINKING_TEXT_KEYS[Math.floor(Math.random() * THINKING_TEXT_KEYS.length)];
    return t(key);
  }, [isStreaming, t]);

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
          isStreaming ? "text-white/60 hover:text-white/80" : "text-white/40 hover:text-white/60",
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
          {isStreaming && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-white/60" />}
          <span className="font-medium">
            {isStreaming ? thinkingText : t("chats.message.thoughtProcess")}
          </span>
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
                "mt-1 pl-5 border-l border-white/10",
                "text-xs text-white/40 italic leading-relaxed",
                "max-h-40 overflow-y-auto",
              )}
            >
              <MarkdownRenderer
                content={reasoning}
                className="text-xs text-white/40 **:text-white/40"
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

function ChatMessageInner({
  message,
  index,
  messagesLength,
  heldMessageId,
  regeneratingMessageId,
  sending,
  eventHandlers,
  getVariantState,
  handleVariantDrag,
  handleRegenerate,
  isStartingSceneMessage,
  theme,
  chatAppearance,
  character,
  persona,
  displayContent,
  onImageClick,
  audioStatus,
  onPlayAudio,
  onStopAudio,
  onCancelAudio,
  reasoning,
  swapPlaces = false,
  modelName,
  scenePromptStreaming = false,
}: ChatMessageProps) {
  const { t } = useI18n();
  const prevRoleRef = useRef(message.role);
  const [crossShift, setCrossShift] = useState(0);

  useLayoutEffect(() => {
    const prevRole = prevRoleRef.current;
    const nextRole = message.role;
    const isPrevSwappable = prevRole === "user" || prevRole === "assistant";
    const isNextSwappable = nextRole === "user" || nextRole === "assistant";
    if (isPrevSwappable && isNextSwappable && prevRole !== nextRole) {
      const prevSide = prevRole === "user" ? 1 : -1;
      const nextSide = nextRole === "user" ? 1 : -1;
      const viewport = typeof window !== "undefined" ? window.innerWidth : 1000;
      const travel = Math.max(240, Math.round(viewport * 0.36));
      const offset = (prevSide - nextSide) * travel;
      setCrossShift(offset);
      prevRoleRef.current = nextRole;
      const raf = requestAnimationFrame(() => {
        setCrossShift(0);
      });
      return () => cancelAnimationFrame(raf);
    }
    prevRoleRef.current = nextRole;
  }, [message.role]);

  // Memoize all computed values
  const computed = useMemo(() => {
    const isAssistant = message.role === "assistant";
    const isScene = message.role === "scene";
    const isUser = message.role === "user";
    const isVisibleSystem = message.role === "system" && Boolean(message.visibleInChat);
    const isPlaceholder = message.id.startsWith("placeholder");
    const actionable = (isAssistant || isUser || isScene || isVisibleSystem) && !isPlaceholder;
    const isLatestAssistant = isAssistant && actionable && index === messagesLength - 1;
    const variantState = getVariantState(message);
    const totalVariants = variantState.total || (isAssistant || isScene ? 1 : 0);
    const selectedVariantIndex =
      variantState.selectedIndex >= 0
        ? variantState.selectedIndex
        : totalVariants > 0
          ? totalVariants - 1
          : -1;

    const enableSwipe = isStartingSceneMessage
      ? index === 0 && variantState.total > 1
      : isLatestAssistant && (variantState.variants?.length ?? 0) > 1;

    const showTypingIndicator = isAssistant && isPlaceholder && message.content.trim().length === 0;
    const showRegenerateButton = isLatestAssistant && !isStartingSceneMessage;
    const shouldAnimate = !isPlaceholder;

    return {
      isAssistant,
      isScene,
      isUser,
      isVisibleSystem,
      isPlaceholder,
      isLatestAssistant,
      totalVariants,
      selectedVariantIndex,
      enableSwipe,
      showTypingIndicator,
      showRegenerateButton,
      shouldAnimate,
    };
  }, [
    message.role,
    message.visibleInChat,
    message.id,
    message.content,
    index,
    messagesLength,
    getVariantState,
    isStartingSceneMessage,
  ]);

  const [mockVariantMode, setMockVariantMode] = useState(false);

  useEffect(() => {
    if (!computed.isLatestAssistant) return;

    const handler = (e: Event) => {
      const { stepId } = (e as CustomEvent).detail;
      setMockVariantMode(stepId === "chat-variants");
    };
    window.addEventListener("tour:step", handler);
    return () => window.removeEventListener("tour:step", handler);
  }, [computed.isLatestAssistant]);

  const effectiveCharName = swapPlaces ? (persona?.title ?? "") : (character?.name ?? "");
  const effectivePersonaName = swapPlaces
    ? (character?.name ?? t("chats.message.userAuthor"))
    : (persona?.title ?? t("chats.message.userAuthor"));
  const resolvedDisplayContent =
    displayContent ?? replacePlaceholders(message.content, effectiveCharName, effectivePersonaName);

  const messageHeaderPlacement = chatAppearance?.messageHeaderPlacement ?? "inside";
  const showMessageHeader = !!(
    chatAppearance?.showMessageAuthor || chatAppearance?.showMessageTimestamp
  );
  const headerAuthorName = computed.isUser
    ? effectivePersonaName
    : computed.isAssistant || computed.isScene
      ? effectiveCharName
      : t("chats.message.systemAuthor");
  const headerRightAligned = message.role === "user" || computed.isVisibleSystem;
  const messageHeaderNode = showMessageHeader ? (
    <div
      className={cn(
        "mb-1 flex items-baseline gap-2",
        headerRightAligned ? "justify-end" : "justify-start",
      )}
    >
      {chatAppearance?.showMessageAuthor && headerAuthorName && (
        <span className="text-[13px] font-semibold">{headerAuthorName}</span>
      )}
      {chatAppearance?.showMessageTimestamp && (
        <span className="text-[11px] font-normal opacity-50">
          {formatMessageTimestamp(
            message.createdAt,
            chatAppearance?.timestampFormat ?? "relative",
            t,
          )}
        </span>
      )}
    </div>
  ) : null;
  const headerAbove = messageHeaderPlacement === "above" && !!messageHeaderNode;

  const messageUsage = message.usage;
  const infoEligible = computed.isAssistant || computed.isScene;
  const showInfoModel = infoEligible && !!chatAppearance?.showMessageModel && !!modelName;
  const showInfoInput =
    infoEligible &&
    !!chatAppearance?.showMessageInputTokens &&
    typeof messageUsage?.promptTokens === "number";
  const showInfoOutput =
    infoEligible &&
    !!chatAppearance?.showMessageOutputTokens &&
    typeof messageUsage?.completionTokens === "number";
  const showInfoTotal =
    infoEligible &&
    !!chatAppearance?.showMessageTotalTokens &&
    typeof messageUsage?.totalTokens === "number";
  const showInfoTtft =
    infoEligible &&
    !!chatAppearance?.showMessageTtft &&
    typeof messageUsage?.firstTokenMs === "number";
  const showInfoTps =
    infoEligible &&
    !!chatAppearance?.showMessageTokensPerSecond &&
    typeof messageUsage?.tokensPerSecond === "number";
  const hasMessageInfo =
    showInfoModel ||
    showInfoInput ||
    showInfoOutput ||
    showInfoTotal ||
    showInfoTtft ||
    showInfoTps;
  const messageInfoSize = chatAppearance?.messageInfoSize ?? "small";
  const messageInfoNode = hasMessageInfo ? (
    <div
      className={cn(
        "flex items-center gap-x-2 tabular-nums text-fg/45",
        MESSAGE_INFO_SIZE_MAP[messageInfoSize],
      )}
    >
      {showInfoInput && (
        <span title={t("chats.actions.promptTokens")}>↓&#8202;{messageUsage?.promptTokens}</span>
      )}
      {showInfoOutput && (
        <span title={t("chats.actions.completionTokens")}>↑&#8202;{messageUsage?.completionTokens}</span>
      )}
      {showInfoModel && <span className="min-w-0 truncate text-fg/55">{modelName}</span>}
      {showInfoTtft && (
        <span className="whitespace-nowrap" title={t("chats.actions.timeToFirstToken")}>
          TTFT {messageUsage?.firstTokenMs}ms
        </span>
      )}
      {showInfoTps && (
        <span className="whitespace-nowrap" title={t("chats.actions.completionTokenSpeed")}>
          {messageUsage?.tokensPerSecond?.toFixed(1)} tok/s
        </span>
      )}
      {showInfoTotal && (
        <span className="whitespace-nowrap">
          {(messageUsage?.totalTokens ?? 0).toLocaleString()}
          <span className="ml-1 opacity-50">{t("chats.actions.total")}</span>
        </span>
      )}
    </div>
  ) : null;
  const messageInfoPlacement = chatAppearance?.messageInfoPlacement ?? "belowBubble";
  const infoBelowHeader = messageInfoPlacement === "belowHeader" ? messageInfoNode : null;
  const infoBelowHeaderOutside =
    messageInfoPlacement === "belowHeaderOutside" ? messageInfoNode : null;
  const infoInsideBubble = messageInfoPlacement === "insideBubble" ? messageInfoNode : null;
  const infoBelowBubble = messageInfoPlacement === "belowBubble" ? messageInfoNode : null;
  const bubbleMaxWidthClass = chatAppearance
    ? BUBBLE_MAX_WIDTH_MAP[chatAppearance.bubbleMaxWidth]
    : "max-w-[82%]";
  const bubbleColumnWrapped = headerAbove || !!infoBelowHeaderOutside || !!infoBelowBubble;
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
    resolvedDisplayContent.trim().length > 0;
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
        await onPlayAudio(message, resolvedDisplayContent);
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
      resolvedDisplayContent,
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

  // Load attachments with lazy loading support for persisted images
  const loadedAttachments = useSessionAttachments(message.attachments);
  const bubbleStyle =
    chatAppearance?.bubbleStyle === "minimal"
      ? undefined
      : message.role === "user" || computed.isVisibleSystem
        ? {
            backgroundColor: theme.userBgColor,
            borderColor: theme.userBorderColor,
          }
        : {
            backgroundColor: theme.assistantBgColor,
            borderColor: theme.assistantBorderColor,
          };

  return (
    <motion.div
      layout="position"
      animate={{ x: crossShift }}
      transition={
        swapPlaces
          ? {
              x: { type: "tween", duration: 0.42, ease: [0.22, 0.61, 0.36, 1] },
              layout: { type: "spring", stiffness: 260, damping: 28, mass: 0.85 },
            }
          : { x: { duration: 0.16, ease: "easeOut" }, layout: { duration: 0.16, ease: "easeOut" } }
      }
      className={cn(
        "relative flex gap-2",
        message.role === "user" || computed.isVisibleSystem ? "justify-end" : "justify-start",
      )}
    >
      {/* Avatar for assistant/scene messages (left side) */}
      {(message.role === "assistant" || message.role === "scene") &&
        chatAppearance?.avatarShape !== "hidden" && (
          <div className="flex shrink-0 flex-col items-center gap-1">
            <MessageAvatar
              role={message.role}
              character={character}
              persona={persona}
              avatarShape={chatAppearance?.avatarShape}
              avatarSize={chatAppearance?.avatarSize}
            />
            {canPlayAudio && (
              <button
                type="button"
                onClick={handlePlayAudio}
                className={cn(
                  "audio-btn flex h-6 w-6 items-center justify-center rounded-full",
                  "border border-white/40 bg-white/10 text-white shadow-sm",
                  "transition hover:bg-white/20 active:scale-95",
                  "disabled:cursor-not-allowed disabled:opacity-70",
                )}
                aria-label={
                  isAudioLoading
                    ? t("chats.message.cancelAudioGeneration")
                    : isAudioPlaying
                      ? t("chats.message.stopAudio")
                      : t("chats.message.playMessageAudio")
                }
                aria-pressed={isAudioPlaying}
                aria-busy={isAudioLoading}
                title={
                  isAudioLoading
                    ? t("chats.message.cancelAudioGeneration")
                    : isAudioPlaying
                      ? t("chats.message.stopAudio")
                      : t("chats.message.playAudio")
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

      <BubbleHeaderWrap
        above={headerAbove}
        rightAligned={headerRightAligned}
        header={messageHeaderNode}
        aboveBubble={infoBelowHeaderOutside}
        footer={infoBelowBubble}
        maxWidthClass={bubbleMaxWidthClass}
      >
      <motion.div
        {...(computed.isLatestAssistant ? { "data-tour-id": "chat-message-bubble" } : {})}
        initial={computed.shouldAnimate ? { opacity: 0, y: 4 } : false}
        animate={computed.shouldAnimate ? { opacity: 1, y: 0 } : { opacity: 1, y: 0 }}
        transition={animTransition}
        className={cn(
          bubbleColumnWrapped ? "max-w-full" : bubbleMaxWidthClass,
          chatAppearance ? BUBBLE_PADDING_MAP[chatAppearance.bubblePadding] : "px-4 py-2.5",
          chatAppearance ? LINE_SPACING_MAP[chatAppearance.lineSpacing] : "leading-relaxed",
          chatAppearance ? BUBBLE_RADIUS_MAP[chatAppearance.bubbleRadius] : radius.lg,
          chatAppearance ? FONT_SIZE_MAP[chatAppearance.fontSize] : typography.body.size,
          message.role === "user" || computed.isVisibleSystem
            ? cn(
                "ml-auto",
                chatAppearance?.bubbleStyle === "minimal"
                  ? `${theme.userText}`
                  : `${theme.userBg} ${theme.userText}`,
                chatAppearance?.bubbleStyle === "bordered" && `border ${theme.userBorder}`,
                heldMessageId === message.id && "ring-2 ring-accent/50",
              )
            : cn(
                chatAppearance?.bubbleStyle === "minimal"
                  ? `${theme.assistantText}`
                  : `${theme.assistantBg} ${theme.assistantText}`,
                chatAppearance?.bubbleStyle === "bordered" &&
                  (heldMessageId === message.id
                    ? "border border-white/30"
                    : `border ${theme.assistantBorder}`),
                chatAppearance?.bubbleStyle === "filled" &&
                  heldMessageId === message.id &&
                  "ring-2 ring-white/30",
              ),
        )}
        style={bubbleStyle}
        {...eventHandlers}
        {...dragProps}
      >
        {/* Pin indicator */}
        {message.isPinned && (
          <motion.div
            className="absolute -top-2 -right-2 z-10 flex h-5 w-5 items-center justify-center rounded-full border border-blue-500/40 bg-blue-500/20 shadow-lg"
            initial={{ scale: 0, rotate: -45 }}
            animate={{ scale: 1, rotate: 0 }}
            transition={{ type: "spring", stiffness: 300, damping: 20 }}
          >
            <Pin size={12} className="text-blue-300" />
          </motion.div>
        )}

        {messageHeaderPlacement === "inside" && messageHeaderNode}

        {infoBelowHeader && <div className="mb-1.5">{infoBelowHeader}</div>}

        {/* Thinking/Reasoning section - shown even during typing indicator */}
        {computed.isAssistant && reasoning && (
          <ThinkingSection reasoning={reasoning} isStreaming={computed.isPlaceholder && sending} />
        )}

        {/* Show typing indicator only if no reasoning (reasoning section has its own indicator) */}
        {computed.showTypingIndicator && !reasoning ? (
          <TypingIndicator />
        ) : computed.showTypingIndicator ? null : (
          <>
            <SceneContent
              key={message.id + ":" + computed.selectedVariantIndex}
              content={resolvedDisplayContent}
              className="text-inherit select-none"
              onImageClick={onImageClick}
              textColors={
                chatAppearance?.messageTextColorHex ||
                chatAppearance?.plainTextColorHex ||
                chatAppearance?.italicTextColorHex ||
                chatAppearance?.quotedTextColorHex ||
                chatAppearance?.inlineCodeTextColorHex
                  ? {
                      texts: chatAppearance.messageTextColorHex ?? chatAppearance.plainTextColorHex,
                      plain: chatAppearance.plainTextColorHex,
                      italic: chatAppearance.italicTextColorHex,
                      quoted: chatAppearance.quotedTextColorHex,
                      code: chatAppearance.inlineCodeTextColorHex,
                    }
                  : undefined
              }
            />

            {/* Display attachments below the message content */}
            {loadedAttachments.length > 0 && (
              <div className="mt-3 flex flex-wrap gap-2">
                {loadedAttachments.map((attachment) => {
                  const isAudio = attachment.mimeType?.startsWith("audio/");
                  if (isAudio) {
                    return (
                      <AudioAttachmentPlayer
                        key={attachment.id}
                        src={attachment.data}
                        filename={attachment.filename}
                        fallbackLabel={t("chats.message.attachedAudio")}
                        className={cn(
                          radius.md,
                          "w-72 border border-white/15 bg-white/5 px-3 py-3",
                        )}
                        buttonClassName="bg-accent text-black"
                      />
                    );
                  }
                  return (
                    <div
                      key={attachment.id}
                      className={cn(
                        radius.md,
                        "overflow-hidden border border-white/15",
                        attachment.data &&
                          onImageClick &&
                          "cursor-pointer hover:border-white/30 transition-colors",
                      )}
                      onClick={() =>
                        attachment.data &&
                        onImageClick?.(
                          attachment.data,
                          attachment.filename || t("chats.message.attachedImage"),
                        )
                      }
                    >
                      {attachment.data ? (
                        <img
                          src={attachment.data}
                          alt={attachment.filename || t("chats.message.attachedImage")}
                          className="max-h-48 max-w-full object-contain"
                          style={{
                            maxWidth:
                              attachment.width && attachment.width > 300
                                ? 300
                                : attachment.width || 300,
                          }}
                        />
                      ) : (
                        <div
                          className="flex items-center justify-center bg-white/5"
                          style={{
                            width: Math.min(attachment.width || 150, 300),
                            height: Math.min(attachment.height || 100, 192),
                          }}
                        >
                          <div className="h-5 w-5 animate-spin rounded-full border-2 border-white/20 border-t-white/60" />
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </>
        )}

        {(computed.isAssistant || computed.isScene) && computed.totalVariants > 1 && (
          <motion.div
            className={cn(
              "mt-2.5 flex items-center justify-between pr-2",
              typography.caption.size,
              typography.caption.weight,
              "uppercase tracking-wider text-white/40",
            )}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.2, delay: 0.15 }}
          >
            <span className="text-white">
              {isStartingSceneMessage
                ? t("chats.message.sceneLabel")
                : t("chats.message.variantLabel")}{" "}
              {computed.selectedVariantIndex >= 0 ? computed.selectedVariantIndex + 1 : 1}
              {computed.totalVariants > 0 ? ` / ${computed.totalVariants}` : ""}
            </span>
            {regeneratingMessageId === message.id && (
              <motion.span
                className="flex items-center gap-1.5 text-emerald-300"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ duration: 0.15 }}
              >
                <span className="h-3 w-3 animate-spin rounded-full border-2 border-emerald-300/30 border-t-emerald-300" />
                {t("chats.message.regenerating")}
              </motion.span>
            )}
          </motion.div>
        )}

        {scenePromptStreaming && (
          <motion.div
            className={cn(
              "mt-2.5 flex items-center gap-1.5 pr-2",
              typography.caption.size,
              typography.caption.weight,
              "uppercase tracking-wider text-emerald-300",
            )}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.2 }}
          >
            <span className="h-3 w-3 animate-spin rounded-full border-2 border-emerald-300/30 border-t-emerald-300" />
            {t("chats.message.generatingImagePrompt")}
          </motion.div>
        )}

        {mockVariantMode && computed.totalVariants <= 1 && (
          <motion.div
            data-tour-id="chat-variants"
            className={cn(
              "mt-2.5 flex items-center justify-between pr-2",
              typography.caption.size,
              typography.caption.weight,
              "uppercase tracking-wider text-white/40",
            )}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.2 }}
          >
            <span className="text-white">
              {t("chats.message.variantLabel")} 2 / 3
            </span>
          </motion.div>
        )}
        {infoInsideBubble && <div className="mt-2">{infoInsideBubble}</div>}
      </motion.div>
      </BubbleHeaderWrap>

      {(message.role === "user" || computed.isVisibleSystem) &&
        chatAppearance?.avatarShape !== "hidden" && (
        <MessageAvatar
          role={computed.isVisibleSystem ? "system" : message.role}
          character={character}
          persona={persona}
          avatarShape={chatAppearance?.avatarShape}
          avatarSize={chatAppearance?.avatarSize}
        />
      )}

      {computed.showRegenerateButton && (
        <MessageActions
          disabled={regeneratingMessageId === message.id || sending}
          isRegenerating={regeneratingMessageId === message.id}
          onRegenerate={() => void handleRegenerate(message)}
          onGuidedRegenerate={(guidance) => void handleRegenerate(message, { guidance })}
        />
      )}
    </motion.div>
  );
}

export const ChatMessage = React.memo(ChatMessageInner, (prev, next) => {
  const a = prev.message;
  const b = next.message;
  const aAttachments = a.attachments ?? [];
  const bAttachments = b.attachments ?? [];

  const attachmentDataFingerprint = (value?: string | null) =>
    value ? `${value.length}:${value.slice(0, 64)}:${value.slice(-32)}` : "";

  const areAttachmentsEqual = (() => {
    if (aAttachments === bAttachments) return true;
    if (aAttachments.length !== bAttachments.length) return false;
    return aAttachments.every((att, idx) => {
      const other = bAttachments[idx];
      return (
        att.id === other?.id &&
        att.storagePath === other?.storagePath &&
        att.mimeType === other?.mimeType &&
        att.filename === other?.filename &&
        att.width === other?.width &&
        att.height === other?.height &&
        attachmentDataFingerprint(att.data) === attachmentDataFingerprint(other?.data)
      );
    });
  })();

  return (
    a.id === b.id &&
    a.role === b.role &&
    a.visibleInChat === b.visibleInChat &&
    a.content === b.content &&
    a.selectedVariantId === b.selectedVariantId &&
    (a.variants?.length ?? 0) === (b.variants?.length ?? 0) &&
    areAttachmentsEqual &&
    prev.index === next.index &&
    prev.messagesLength === next.messagesLength &&
    prev.heldMessageId === next.heldMessageId &&
    prev.regeneratingMessageId === next.regeneratingMessageId &&
    prev.sending === next.sending &&
    prev.isStartingSceneMessage === next.isStartingSceneMessage &&
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
    prev.displayContent === next.displayContent &&
    prev.modelName === next.modelName &&
    prev.scenePromptStreaming === next.scenePromptStreaming &&
    prev.swapPlaces === next.swapPlaces &&
    prev.audioStatus === next.audioStatus &&
    a.reasoning === b.reasoning &&
    prev.reasoning === next.reasoning &&
    prev.onPlayAudio === next.onPlayAudio &&
    prev.onStopAudio === next.onStopAudio &&
    prev.onCancelAudio === next.onCancelAudio
  );
});

function TypingIndicator() {
  const { t } = useI18n();
  return (
    <div
      className="flex items-center gap-1"
      aria-label={t("chats.message.assistantIsTyping")}
      aria-live="polite"
    >
      <span className="typing-dot" />
      <span className="typing-dot" style={{ animationDelay: "0.2s" }} />
      <span className="typing-dot" style={{ animationDelay: "0.4s" }} />
    </div>
  );
}
