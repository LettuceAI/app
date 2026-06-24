import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ArrowUp, Check, ChevronsRight, Mic, Plus, Square, X } from "lucide-react";
import type { Character, ImageAttachment } from "../../../../core/storage/schemas";
import { radius, typography, interactive, shadows, cn } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { useI18n } from "../../../../core/i18n/context";
import { BottomMenu } from "../../../components/BottomMenu";
import { ChatErrorBanner } from "./ChatErrorBanner";
import { AudioAttachmentPlayer } from "./AudioAttachmentPlayer";

const SYSTEM_SEND_CONFIRMATION_DISABLED_STORAGE_KEY =
  "lettuce.chat.systemSendConfirmationDisabled";

interface ChatFooterProps {
  draft: string;
  setDraft: (value: string) => void;
  error: string | null;
  sending: boolean;
  character: Character;
  onSendMessage: () => Promise<void>;
  onSendSystemMessage?: () => Promise<void>;
  onAbort?: () => Promise<void>;
  hasBackgroundImage?: boolean;
  footerOverlayClassName?: string;
  footerOverlayColor?: string;
  footerFgColor?: string;
  footerFgMutedColor?: string;
  pendingAttachments?: ImageAttachment[];
  onAddAttachment?: (attachment: ImageAttachment) => void;
  onRemoveAttachment?: (attachmentId: string) => void;
  onOpenPlusMenu?: () => void;
  triggerFileInput?: boolean;
  onFileInputTriggered?: () => void;
  triggerAudioInput?: boolean;
  onAudioInputTriggered?: () => void;
  textareaRef?: React.RefObject<HTMLTextAreaElement | null>;
  topSlot?: ReactNode;
  inlinePanel?: ReactNode;
  onMicClick?: () => void;
  onMicCancel?: () => void;
  micActive?: boolean;
  micDisabled?: boolean;
  recordingElapsedMs?: number;
  recordingAnalyser?: AnalyserNode | null;
  recordingTranscribing?: boolean;
  composerDisabled?: boolean;
}

export function ChatFooter({
  draft,
  setDraft,
  error,
  sending,
  onSendMessage,
  onSendSystemMessage,
  onAbort,
  hasBackgroundImage,
  footerOverlayClassName,
  footerOverlayColor,
  footerFgColor,
  footerFgMutedColor,
  pendingAttachments = [],
  onAddAttachment,
  onRemoveAttachment,
  onOpenPlusMenu,
  triggerFileInput,
  onFileInputTriggered,
  triggerAudioInput,
  onAudioInputTriggered,
  textareaRef: externalTextareaRef,
  topSlot,
  inlinePanel,
  onMicClick,
  onMicCancel,
  micActive = false,
  micDisabled = false,
  recordingElapsedMs = 0,
  recordingAnalyser = null,
  recordingTranscribing = false,
  composerDisabled = false,
}: ChatFooterProps) {
  const { t } = useI18n();
  const hasDraft = draft.trim().length > 0;
  const hasAttachments = pendingAttachments.length > 0;
  const hasFooterColor = !!footerOverlayColor;
  const footerIconIdle = hasFooterColor ? "text-[var(--footer-fg-muted)]" : "text-fg/60";
  const footerIconHover = hasFooterColor ? "hover:text-[var(--footer-fg)]" : "hover:text-fg";
  const fileInputRef = useRef<HTMLInputElement>(null);
  const audioInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendLongPressTimerRef = useRef<number | null>(null);
  const sendLongPressTriggeredRef = useRef(false);
  const [showSystemSendMenu, setShowSystemSendMenu] = useState(false);
  const [sendingSystemMessage, setSendingSystemMessage] = useState(false);
  const [skipSystemSendConfirmation, setSkipSystemSendConfirmation] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(SYSTEM_SEND_CONFIRMATION_DISABLED_STORAGE_KEY) === "1";
  });

  useEffect(() => {
    if (!externalTextareaRef) return;
    externalTextareaRef.current = textareaRef.current;
    return () => {
      if (externalTextareaRef.current === textareaRef.current) {
        externalTextareaRef.current = null;
      }
    };
  }, [externalTextareaRef]);

  const isDesktop = useMemo(() => getPlatform().type === "desktop", []);
  const canOpenSystemSendMenu =
    !sending && !composerDisabled && (hasDraft || hasAttachments) && Boolean(onSendSystemMessage);

  const clearSendLongPressTimer = useCallback(() => {
    if (sendLongPressTimerRef.current !== null) {
      window.clearTimeout(sendLongPressTimerRef.current);
      sendLongPressTimerRef.current = null;
    }
  }, []);

  const openSystemSendMenu = useCallback(() => {
    if (!canOpenSystemSendMenu) return;
    sendLongPressTriggeredRef.current = true;
    if (skipSystemSendConfirmation) {
      void onSendSystemMessage?.();
      return;
    }
    setShowSystemSendMenu(true);
  }, [canOpenSystemSendMenu, onSendSystemMessage, skipSystemSendConfirmation]);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [draft]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (!isDesktop) return;

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!sending && !composerDisabled && (hasDraft || hasAttachments)) {
        onSendMessage();
      }
    }
  };

  const handleFileSelect = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (!files || !onAddAttachment) return;

    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;

      const reader = new FileReader();
      reader.onload = () => {
        const base64 = reader.result as string;

        // Create image to get dimensions
        const img = new Image();
        img.onload = () => {
          const attachment: ImageAttachment = {
            id: crypto.randomUUID(),
            data: base64,
            mimeType: file.type,
            filename: file.name,
            width: img.width,
            height: img.height,
          };
          onAddAttachment(attachment);
        };
        img.src = base64;
      };
      reader.readAsDataURL(file);
    }

    event.target.value = "";
  };

  const handleAudioFileSelect = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (!files || !onAddAttachment) return;

    for (const file of Array.from(files)) {
      if (!file.type.startsWith("audio/")) continue;

      const reader = new FileReader();
      reader.onload = () => {
        const base64 = reader.result as string;
        onAddAttachment({
          id: crypto.randomUUID(),
          data: base64,
          mimeType: file.type,
          filename: file.name,
        });
      };
      reader.readAsDataURL(file);
    }

    event.target.value = "";
  };

  const handlePlusClick = () => {
    if (onOpenPlusMenu) {
      onOpenPlusMenu();
    } else {
      fileInputRef.current?.click();
    }
  };

  useEffect(() => {
    if (triggerFileInput) {
      fileInputRef.current?.click();
      onFileInputTriggered?.();
    }
  }, [triggerFileInput, onFileInputTriggered]);

  useEffect(() => {
    if (triggerAudioInput) {
      audioInputRef.current?.click();
      onAudioInputTriggered?.();
    }
  }, [triggerAudioInput, onAudioInputTriggered]);

  useEffect(() => clearSendLongPressTimer, [clearSendLongPressTimer]);

  const handleSendButtonPointerDown = useCallback(
    (event: React.PointerEvent<HTMLButtonElement>) => {
      if (!canOpenSystemSendMenu || event.button !== 0) return;
      clearSendLongPressTimer();
      sendLongPressTriggeredRef.current = false;
      sendLongPressTimerRef.current = window.setTimeout(openSystemSendMenu, 450);
    },
    [canOpenSystemSendMenu, clearSendLongPressTimer, openSystemSendMenu],
  );

  const handleSendButtonPointerEnd = useCallback(() => {
    clearSendLongPressTimer();
  }, [clearSendLongPressTimer]);

  const handleSendButtonClick = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      clearSendLongPressTimer();
      if (sendLongPressTriggeredRef.current) {
        event.preventDefault();
        event.stopPropagation();
        window.setTimeout(() => {
          sendLongPressTriggeredRef.current = false;
        }, 0);
        return;
      }
      void (sending && onAbort ? onAbort() : onSendMessage());
    },
    [clearSendLongPressTimer, onAbort, onSendMessage, sending],
  );

  const handleSendButtonContextMenu = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      if (!canOpenSystemSendMenu) return;
      event.preventDefault();
      event.stopPropagation();
      clearSendLongPressTimer();
      openSystemSendMenu();
    },
    [canOpenSystemSendMenu, clearSendLongPressTimer, openSystemSendMenu],
  );

  const handleCloseSystemSendMenu = useCallback(() => {
    setShowSystemSendMenu(false);
    sendLongPressTriggeredRef.current = false;
  }, []);

  const handleConfirmSystemSend = useCallback(async () => {
    if (!onSendSystemMessage || sendingSystemMessage) return;
    setSendingSystemMessage(true);
    try {
      setSkipSystemSendConfirmation(true);
      if (typeof window !== "undefined") {
        window.localStorage.setItem(SYSTEM_SEND_CONFIRMATION_DISABLED_STORAGE_KEY, "1");
      }
      await onSendSystemMessage();
      handleCloseSystemSendMenu();
    } finally {
      setSendingSystemMessage(false);
    }
  }, [handleCloseSystemSendMenu, onSendSystemMessage, sendingSystemMessage]);

  return (
    <>
      <footer
        className={cn(
          "z-20 shrink-0 px-4 pb-6 pt-3",
          hasBackgroundImage ? "bg-transparent" : "bg-surface",
        )}
      >
        {error && <ChatErrorBanner error={error} />}

        {hasAttachments && (
          <div className="mb-2 flex flex-wrap gap-2 overflow-visible p-1">
            {pendingAttachments.map((attachment) => {
              const isAudio = attachment.mimeType?.startsWith("audio/");
              return (
                <div
                  key={attachment.id}
                  className={cn("relative", radius.md, "border border-fg/15 bg-fg/8")}
                >
                  {isAudio ? (
                    <AudioAttachmentPlayer
                      src={attachment.data}
                      filename={attachment.filename}
                      fallbackLabel={t("chats.footer.audioAttachmentLabel")}
                      className="w-72 px-3 py-3"
                      buttonClassName="bg-accent text-black"
                    />
                  ) : (
                    <img
                      src={attachment.data}
                      alt={attachment.filename || t("chats.footer.attachmentAlt")}
                      className={cn("h-20 w-20 object-cover", radius.md)}
                    />
                  )}
                  {onRemoveAttachment && (
                    <button
                      onClick={() => onRemoveAttachment(attachment.id)}
                      className={cn(
                        "absolute -right-1 -top-1 z-50",
                        interactive.transition.fast,
                        interactive.active.scale,
                      )}
                      aria-label={t("chats.footer.removeAttachment")}
                    >
                      <X className="h-5 w-5 text-fg drop-shadow-[0_1px_2px_rgba(0,0,0,0.25)]" />
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        )}

        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          multiple
          className="hidden"
          onChange={handleFileSelect}
        />

        <input
          ref={audioInputRef}
          type="file"
          accept="audio/*"
          multiple
          className="hidden"
          onChange={handleAudioFileSelect}
        />

        <div
          className={cn(
            "relative",
            "rounded-4xl",
            "border border-fg/15 backdrop-blur-md",
            footerOverlayColor
              ? null
              : hasBackgroundImage
                ? footerOverlayClassName || "bg-surface-el/65"
                : "bg-surface-el/65",
            shadows.md,
          )}
          style={
            footerOverlayColor
              ? {
                  backgroundColor: footerOverlayColor,
                  ["--footer-fg" as string]: footerFgColor,
                  ["--footer-fg-muted" as string]: footerFgMutedColor,
                }
              : undefined
          }
        >
          {topSlot && (
            <div className="border-b border-fg/10">
              {topSlot}
            </div>
          )}
          {inlinePanel && <div className="border-b border-fg/10">{inlinePanel}</div>}
          <div className="relative flex items-end gap-2.5 p-2">
        {/* Plus button */}
        {(onOpenPlusMenu || onAddAttachment) && (
          <button
            data-tour-id="chat-plus"
            onClick={handlePlusClick}
            disabled={sending}
            className={cn(
              "mb-0.5 flex h-10.75 w-10.75 shrink-0 items-center justify-center self-end",
              radius.full,
              footerIconIdle,
              interactive.transition.fast,
              interactive.active.scale,
              "hover:bg-fg/10",
              footerIconHover,
              "disabled:cursor-not-allowed disabled:opacity-40",
            )}
            title={onOpenPlusMenu ? t("chats.footer.moreOptions") : t("chats.footer.addImage")}
            aria-label={
              onOpenPlusMenu ? t("chats.footer.moreOptions") : t("chats.footer.addImageAttachment")
            }
          >
            <Plus size={20} />
          </button>
        )}

        {micActive ? (
          <>
            <RecordingIndicator
              elapsedMs={recordingElapsedMs}
              analyser={recordingAnalyser}
              frozen={recordingTranscribing}
            />
            {onMicCancel && (
              <button
                onClick={onMicCancel}
                disabled={recordingTranscribing}
                className={cn(
                  "mb-0.5 flex h-10.75 w-10.75 shrink-0 items-center justify-center self-end",
                  radius.full,
                  "text-fg/60",
                  interactive.transition.fast,
                  interactive.active.scale,
                  "hover:bg-fg/10 hover:text-fg",
                  "disabled:cursor-not-allowed disabled:opacity-40",
                )}
                title={t("chats.footer.cancelRecording")}
                aria-label={t("chats.footer.cancelRecording")}
              >
                <X size={18} />
              </button>
            )}
            {recordingTranscribing ? (
              <div
                className={cn(
                  "mb-0.5 flex h-10.75 w-10.75 shrink-0 items-center justify-center self-end",
                  radius.full,
                  "bg-accent text-black",
                )}
                aria-label={t("chats.footer.transcribing")}
                title={t("chats.footer.transcribing")}
              >
                <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" />
              </div>
            ) : (
              onMicClick && (
                <button
                  onClick={onMicClick}
                  disabled={micDisabled}
                  className={cn(
                    "mb-0.5 flex h-10.75 w-10.75 shrink-0 items-center justify-center self-end",
                    radius.full,
                    "bg-accent text-black shadow-sm",
                    interactive.transition.fast,
                    interactive.active.scale,
                    "hover:brightness-110",
                    "disabled:cursor-not-allowed disabled:opacity-40",
                  )}
                  title={t("chats.footer.stopAndTranscribe")}
                  aria-label={t("chats.footer.stopAndTranscribe")}
                >
                  <Check size={18} strokeWidth={2.75} />
                </button>
              )
            )}
          </>
        ) : (
          <>
            <textarea
              ref={textareaRef}
              data-tour-id="chat-composer"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t("chats.footer.sendMessagePlaceholder")}
              rows={1}
              className={cn(
                "max-h-32 flex-1 resize-none bg-transparent py-2.5",
                typography.body.size,
                hasFooterColor
                  ? "text-[var(--footer-fg)] placeholder:text-[var(--footer-fg-muted)]"
                  : "text-fg placeholder:text-fg/40",
                "focus:outline-none",
              )}
              disabled={sending || composerDisabled}
            />


            {onMicClick && !hasDraft && !hasAttachments && !sending && (
              <button
                onClick={onMicClick}
                disabled={micDisabled}
                className={cn(
                  "mb-0.5 flex h-10.75 w-10.75 shrink-0 items-center justify-center self-end",
                  radius.full,
                  footerIconIdle,
                  interactive.transition.fast,
                  interactive.active.scale,
                  "hover:bg-fg/10",
                  footerIconHover,
                  "disabled:cursor-not-allowed disabled:opacity-40",
                )}
                title={t("chats.footer.recordVoice")}
                aria-label={t("chats.footer.recordVoice")}
              >
                <Mic size={18} strokeWidth={2} />
              </button>
            )}

            <button
              data-tour-id="chat-send"
              onPointerDown={handleSendButtonPointerDown}
              onPointerUp={handleSendButtonPointerEnd}
              onPointerLeave={handleSendButtonPointerEnd}
              onPointerCancel={handleSendButtonPointerEnd}
              onClick={handleSendButtonClick}
              onContextMenu={handleSendButtonContextMenu}
              disabled={(sending && !onAbort) || composerDisabled}
              className={cn(
                "mb-0.5 flex h-10.75 w-10.75 shrink-0 items-center justify-center self-end",
                radius.full,
                interactive.transition.fast,
                interactive.active.scale,
                sending && onAbort
                  ? "bg-red-400/90 text-white hover:brightness-110"
                  : hasDraft || hasAttachments
                    ? "bg-accent text-black shadow-sm hover:brightness-110"
                    : hasFooterColor
                      ? "bg-fg/15 text-[var(--footer-fg-muted)] hover:bg-fg/20"
                      : "bg-fg/15 text-fg/55 hover:bg-fg/20",
                "disabled:cursor-not-allowed disabled:opacity-40",
              )}
              title={
                sending && onAbort
                  ? t("chats.footer.stopGeneration")
                  : hasDraft || hasAttachments
                    ? t("chats.footer.sendMessage")
                    : t("chats.footer.continueConversation")
              }
              aria-label={
                sending && onAbort
                  ? t("chats.footer.stopGeneration")
                  : hasDraft || hasAttachments
                    ? t("chats.footer.sendMessage")
                    : t("chats.footer.continueConversation")
              }
            >
              {sending && onAbort ? (
                <Square size={16} fill="currentColor" />
              ) : sending ? (
                <span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
              ) : hasDraft || hasAttachments ? (
                <ArrowUp size={18} strokeWidth={2.75} />
              ) : (
                <ChevronsRight size={18} />
              )}
            </button>
          </>
        )}
          </div>
        </div>
      </footer>

      <BottomMenu
        isOpen={showSystemSendMenu}
        onClose={handleCloseSystemSendMenu}
        title={t("chats.footer.systemSend.title")}
      >
        <div className="space-y-4">
          <p className="text-sm leading-relaxed text-fg/55">
            {t("chats.footer.systemSend.description")}
          </p>

          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={handleCloseSystemSendMenu}
              disabled={sendingSystemMessage}
              className={cn(
                "h-11 border px-3 text-sm font-semibold transition",
                radius.md,
                "border-fg/10 bg-fg/4 text-fg/75 hover:border-fg/20 hover:bg-fg/[0.07]",
                "disabled:cursor-not-allowed disabled:opacity-45",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              type="button"
              onClick={() => void handleConfirmSystemSend()}
              disabled={sendingSystemMessage}
              className={cn(
                "h-11 border px-3 text-sm font-semibold transition",
                radius.md,
                "border-emerald-400/35 bg-emerald-500/20 text-emerald-100 hover:bg-emerald-500/30",
                "disabled:cursor-not-allowed disabled:opacity-45",
              )}
            >
              {sendingSystemMessage
                ? t("chats.footer.systemSend.sending")
                : t("chats.footer.systemSend.send")}
            </button>
          </div>
        </div>
      </BottomMenu>
    </>
  );
}

function formatElapsed(ms: number) {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export function RecordingIndicator({
  elapsedMs,
  analyser,
  frozen = false,
}: {
  elapsedMs: number;
  analyser: AnalyserNode | null;
  frozen?: boolean;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const samplesRef = useRef<number[]>([]);
  const frameRef = useRef<number | null>(null);
  const lastPushRef = useRef(0);
  const dataRef = useRef<Uint8Array | null>(null);
  const [size, setSize] = useState<{ w: number; h: number }>({ w: 0, h: 0 });

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      const rect = el.getBoundingClientRect();
      setSize({ w: Math.max(0, Math.floor(rect.width)), h: Math.max(0, Math.floor(rect.height)) });
    });
    observer.observe(el);
    const rect = el.getBoundingClientRect();
    setSize({ w: Math.max(0, Math.floor(rect.width)), h: Math.max(0, Math.floor(rect.height)) });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    samplesRef.current = [];
    lastPushRef.current = 0;
    if (analyser) {
      dataRef.current = new Uint8Array(new ArrayBuffer(analyser.fftSize));
    } else {
      dataRef.current = null;
    }
  }, [analyser]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const cssWidth = size.w;
    const cssHeight = size.h || 20;
    if (cssWidth <= 0) return;
    canvas.width = Math.floor(cssWidth * dpr);
    canvas.height = Math.floor(cssHeight * dpr);
    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${cssHeight}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);

    const BAR_W = 2; // bar width
    const GAP = 2; // gap between bars
    const STEP = BAR_W + GAP;
    const SAMPLE_INTERVAL_MS = 50; // ~20 Hz push rate
    const maxBars = Math.ceil(cssWidth / STEP) + 4;
    const MIN_BAR = 3; // minimum visible bar height
    const MAX_BAR = cssHeight - 2;

    const draw = (now: number) => {
      if (!frozen && analyser && dataRef.current) {
        if (now - lastPushRef.current >= SAMPLE_INTERVAL_MS) {
          analyser.getByteTimeDomainData(dataRef.current as Uint8Array<ArrayBuffer>);
          const data = dataRef.current;
          let sumSq = 0;
          for (let i = 0; i < data.length; i++) {
            const v = (data[i] - 128) / 128;
            sumSq += v * v;
          }
          const rms = Math.sqrt(sumSq / data.length); // 0..1 (typical speech ~0.02–0.15 with AGC)
          const amp = Math.min(1, Math.pow(rms * 18, 0.5));
          samplesRef.current.push(amp);
          if (samplesRef.current.length > maxBars) {
            samplesRef.current.splice(0, samplesRef.current.length - maxBars);
          }
          lastPushRef.current = now;
        }
      }

      ctx.clearRect(0, 0, cssWidth, cssHeight);
      const samples = samplesRef.current;
      const centerY = cssHeight / 2;
      const startIdx = Math.max(0, samples.length - maxBars);
      for (let i = startIdx; i < samples.length; i++) {
        const ageFromRight = samples.length - 1 - i; // 0 = newest
        const x = cssWidth - BAR_W - ageFromRight * STEP;
        if (x + BAR_W < 0) break;
        const amp = samples[i];
        const h = Math.max(MIN_BAR, amp * MAX_BAR);
        const ageRatio = ageFromRight / Math.max(1, maxBars - 1);
        const alpha = 0.95 - ageRatio * 0.55;
        ctx.fillStyle = `rgba(244, 252, 248, ${alpha.toFixed(3)})`;
        ctx.fillRect(x, centerY - h / 2, BAR_W, h);
      }

      frameRef.current = window.requestAnimationFrame(draw);
    };
    frameRef.current = window.requestAnimationFrame(draw);
    return () => {
      if (frameRef.current != null) window.cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    };
  }, [analyser, size.w, size.h, frozen]);

  return (
    <div className="flex min-w-0 flex-1 items-center gap-2 py-2.5">
      <div
        ref={containerRef}
        className="relative h-5 min-w-0 flex-1 overflow-hidden"
      >
        <div
          aria-hidden="true"
          className="pointer-events-none absolute left-0 right-0 top-1/2 h-px -translate-y-1/2"
          style={{
            backgroundImage:
              "radial-gradient(circle, currentColor 1px, transparent 1px)",
            backgroundSize: "6px 1px",
            backgroundRepeat: "repeat-x",
            color: "rgba(255,255,255,0.25)",
          }}
        />
        <canvas ref={canvasRef} className="relative block h-full w-full" />
      </div>
      <span className="shrink-0 text-[11px] tabular-nums text-fg/55">{formatElapsed(elapsedMs)}</span>
    </div>
  );
}
