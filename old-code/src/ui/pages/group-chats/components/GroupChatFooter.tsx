import { useEffect, useMemo, useRef, useState, useCallback, type ReactNode } from "react";
import {
  ArrowUp,
  Check,
  ChevronsRight,
  Mic,
  Plus,
  Square,
  X,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useI18n } from "../../../../core/i18n/context";
import type { Character, ImageAttachment, Persona } from "../../../../core/storage/schemas";
import { radius, typography, interactive, shadows, cn } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { useAvatar } from "../../../hooks/useAvatar";
import { AvatarImage } from "../../../components/AvatarImage";
import { RecordingIndicator } from "../../chats/components/ChatFooter";
import { ChatErrorBanner } from "../../chats/components/ChatErrorBanner";
import {
  GroupChatParticipantsBar,
  type ParticipantsBarSize,
  type ParticipantsBarShape,
  type ParticipantsBarBackground,
  type ParticipantsBarGap,
  type ParticipantsBarAlign,
} from "./GroupChatParticipantsBar";

const EMPTY_MUTED_SET: Set<string> = new Set();

interface GroupChatFooterProps {
  draft: string;
  setDraft: (value: string) => void;
  error: string | null;
  setError?: (error: string | null) => void;
  sending: boolean;
  characters: Character[];
  persona?: Persona | null;
  onSendMessage: () => Promise<void>;
  onContinue?: () => Promise<void>;
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
  inlinePanel?: ReactNode;
  onMicClick?: () => void;
  onMicCancel?: () => void;
  micActive?: boolean;
  micDisabled?: boolean;
  recordingElapsedMs?: number;
  recordingAnalyser?: AnalyserNode | null;
  recordingTranscribing?: boolean;
  composerDisabled?: boolean;
  mutedCharacterIds?: Set<string>;
  onToggleMute?: (characterId: string, muted: boolean) => void;
  participantsBarEnabled?: boolean;
  participantsBarSize?: ParticipantsBarSize;
  participantsBarShape?: ParticipantsBarShape;
  participantsBarBackground?: ParticipantsBarBackground;
  participantsBarGap?: ParticipantsBarGap;
  participantsBarAlign?: ParticipantsBarAlign;
  directorMode?: boolean;
  directorBehavior?: "cue" | "action";
  directorSelectedId?: string | null;
  directorWiggleNonce?: number;
  directorHintPosition?: "top" | "bottom" | "hidden";
  onSelectSpeaker?: (characterId: string) => void;
}

export function GroupChatFooter({
  draft,
  setDraft,
  error,
  setError,
  sending,
  characters,
  onSendMessage,
  onContinue,
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
  inlinePanel,
  onMicClick,
  onMicCancel,
  micActive = false,
  micDisabled = false,
  recordingElapsedMs = 0,
  recordingAnalyser = null,
  recordingTranscribing = false,
  composerDisabled = false,
  mutedCharacterIds,
  onToggleMute,
  participantsBarEnabled = true,
  participantsBarSize = "medium",
  participantsBarShape = "round",
  participantsBarBackground = "transparent",
  participantsBarGap = "normal",
  participantsBarAlign = "left",
  directorMode = false,
  directorBehavior = "cue",
  directorSelectedId = null,
  directorWiggleNonce = 0,
  directorHintPosition = "bottom",
  onSelectSpeaker,
}: GroupChatFooterProps) {
  const { t } = useI18n();
  const hasDraft = draft.trim().length > 0;
  const hasAttachments = pendingAttachments.length > 0;
  const hasFooterColor = !!footerOverlayColor;
  const footerIconIdle = hasFooterColor ? "text-[var(--footer-fg-muted)]" : "text-fg/60";
  const footerIconHover = hasFooterColor ? "hover:text-[var(--footer-fg)]" : "hover:text-fg";
  const canSend =
    hasDraft ||
    hasAttachments ||
    (directorMode && directorBehavior === "cue" && !!directorSelectedId);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Mention autocomplete state
  const [showMentionPicker, setShowMentionPicker] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionStartIndex, setMentionStartIndex] = useState(-1);

  const isDesktop = useMemo(() => getPlatform().type === "desktop", []);

  // Filter characters based on mention query
  const filteredCharacters = useMemo(() => {
    if (!mentionQuery) return characters;
    const query = mentionQuery.toLowerCase();
    return characters.filter((c) => {
      const description = `${c.description ?? ""} ${c.definition ?? ""}`.toLowerCase();
      return c.name.toLowerCase().includes(query) || description.includes(query);
    });
  }, [characters, mentionQuery]);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [draft]);

  // Detect @ mentions while typing
  const handleDraftChange = useCallback(
    (value: string) => {
      setDraft(value);

      if (directorMode) return;

      // Check for @ mention trigger
      const textarea = textareaRef.current;
      if (!textarea) return;

      const cursorPos = textarea.selectionStart;
      const textBeforeCursor = value.slice(0, cursorPos);

      // Find the last @ that could be a mention start
      const lastAtIndex = textBeforeCursor.lastIndexOf("@");

      if (lastAtIndex !== -1) {
        const textAfterAt = textBeforeCursor.slice(lastAtIndex + 1);
        // Check if there's no space after @ (still typing the mention)
        // Also check that @ is at start or preceded by space
        const charBeforeAt = lastAtIndex > 0 ? textBeforeCursor[lastAtIndex - 1] : " ";
        const isValidMentionStart =
          charBeforeAt === " " || charBeforeAt === "\n" || lastAtIndex === 0;

        if (isValidMentionStart && !textAfterAt.includes(" ") && !textAfterAt.includes("\n")) {
          // Check if it's a quoted mention
          if (textAfterAt.startsWith('"')) {
            const closingQuote = textAfterAt.indexOf('"', 1);
            if (closingQuote === -1) {
              // Still typing quoted mention
              setShowMentionPicker(true);
              setMentionQuery(textAfterAt.slice(1));
              setMentionStartIndex(lastAtIndex);
              return;
            }
          } else {
            // Unquoted mention
            setShowMentionPicker(true);
            setMentionQuery(textAfterAt);
            setMentionStartIndex(lastAtIndex);
            return;
          }
        }
      }

      // No active mention
      setShowMentionPicker(false);
      setMentionQuery("");
      setMentionStartIndex(-1);
    },
    [setDraft, directorMode],
  );

  // Insert selected character mention
  const insertMention = useCallback(
    (character: Character) => {
      if (mentionStartIndex === -1) return;

      const beforeMention = draft.slice(0, mentionStartIndex);
      const afterCursor = textareaRef.current
        ? draft.slice(textareaRef.current.selectionStart)
        : "";

      // Use quotes if name has spaces
      const mentionText = character.name.includes(" ")
        ? `@"${character.name}"`
        : `@${character.name}`;

      const newDraft = `${beforeMention}${mentionText} ${afterCursor}`;
      setDraft(newDraft);

      // Close picker
      setShowMentionPicker(false);
      setMentionQuery("");
      setMentionStartIndex(-1);

      // Focus textarea and move cursor after mention
      setTimeout(() => {
        if (textareaRef.current) {
          const newCursorPos = beforeMention.length + mentionText.length + 1;
          textareaRef.current.focus();
          textareaRef.current.setSelectionRange(newCursorPos, newCursorPos);
        }
      }, 10);
    },
    [draft, mentionStartIndex, setDraft],
  );

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Handle mention picker navigation
    if (showMentionPicker && filteredCharacters.length > 0) {
      if (event.key === "Escape") {
        event.preventDefault();
        setShowMentionPicker(false);
        return;
      }
      if (event.key === "Tab" || (event.key === "Enter" && !event.shiftKey)) {
        event.preventDefault();
        insertMention(filteredCharacters[0]);
        return;
      }
    }

    if (!isDesktop) return;

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!sending && (canSend)) {
        onSendMessage();
      } else if (!sending && onContinue && !hasDraft && !hasAttachments) {
        onContinue();
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

  const handlePlusClick = () => {
    if (onOpenPlusMenu) {
      onOpenPlusMenu();
    } else {
      fileInputRef.current?.click();
    }
  };

  const handleSendClick = () => {
    if (sending && onAbort) {
      onAbort();
    } else if (canSend) {
      onSendMessage();
    } else if (onContinue) {
      onContinue();
    }
  };

  const sendButtonLabel =
    sending && onAbort
      ? t("groupChats.footer.stopGeneration")
      : canSend
        ? directorMode && directorBehavior === "action"
          ? t("groupChats.footer.directorAddMessage")
          : t("groupChats.footer.sendMessage")
        : onContinue
          ? t("groupChats.footer.continueConversation")
          : t("groupChats.footer.sendMessage");

  useEffect(() => {
    if (triggerFileInput) {
      fileInputRef.current?.click();
      onFileInputTriggered?.();
    }
  }, [triggerFileInput, onFileInputTriggered]);

  // Close mention picker when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".mention-picker") && !target.closest("textarea")) {
        setShowMentionPicker(false);
      }
    };

    if (showMentionPicker) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [showMentionPicker]);

  return (
    <footer
      className={cn(
        "relative z-20 shrink-0 px-4 pb-3 pt-3",
        hasBackgroundImage ? "bg-transparent" : "bg-surface",
      )}
    >
      {error && (
        <ChatErrorBanner error={error} onDismiss={setError ? () => setError(null) : undefined} />
      )}

      {/* Attachment Preview */}
      {hasAttachments && (
        <div className="mb-2 flex flex-wrap gap-2 overflow-visible p-1">
          {pendingAttachments.map((attachment) => (
            <div
              key={attachment.id}
              className={cn("relative", radius.md, "border border-fg/20 bg-fg/10")}
            >
              <img
                src={attachment.data}
                alt={attachment.filename || "Attachment"}
                className={cn("h-20 w-20 object-cover", radius.md)}
              />
              {onRemoveAttachment && (
                <button
                  onClick={() => onRemoveAttachment(attachment.id)}
                  className={cn(
                    "absolute -right-1 -top-1 z-50",
                    interactive.transition.fast,
                    interactive.active.scale,
                  )}
                  aria-label={t("groupChats.footer.removeAttachment")}
                >
                  <X className="h-5 w-5 text-surface drop-shadow-[0_1px_2px_rgba(255,255,255,0.8)]" />
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Mention Autocomplete Picker */}
      <AnimatePresence>
        {showMentionPicker && filteredCharacters.length > 0 && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            transition={{ duration: 0.15 }}
            className={cn(
              "mention-picker mb-2 max-h-48 overflow-y-auto",
              radius.lg,
              "border border-fg/15 bg-surface-el/95 backdrop-blur-md",
              shadows.lg,
            )}
          >
            <div className="p-1.5">
              <div className="px-2 py-1.5 text-[10px] uppercase tracking-wider text-fg/40 font-medium">
                {t("groupChats.footer.mentionCharacter")}
              </div>
              {filteredCharacters.map((character) => (
                <MentionPickerItem
                  key={character.id}
                  character={character}
                  onClick={() => insertMention(character)}
                  query={mentionQuery}
                />
              ))}
              {filteredCharacters.length === 0 && (
                <div className="px-3 py-4 text-center text-sm text-fg/40">
                  {t("groupChats.footer.noCharactersFound")}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Hidden file input */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        className="hidden"
        onChange={handleFileSelect}
      />

      {!micActive && onToggleMute && (participantsBarEnabled || directorMode) && (
        <div className="pointer-events-none absolute inset-x-0 bottom-full">
          {participantsBarBackground !== "transparent" && (
            <div
              aria-hidden
              className={cn(
                "absolute inset-0",
                participantsBarBackground === "solid"
                  ? hasBackgroundImage
                    ? "bg-black/55"
                    : "bg-surface"
                  : hasBackgroundImage
                    ? "bg-gradient-to-t from-black/45 via-black/20 to-transparent"
                    : "bg-gradient-to-t from-surface via-surface/70 to-transparent",
              )}
            />
          )}
          <div className="pointer-events-auto relative px-4">
            <GroupChatParticipantsBar
              characters={characters}
              draft={draft}
              setDraft={setDraft}
              mutedCharacterIds={mutedCharacterIds ?? EMPTY_MUTED_SET}
              onToggleMute={onToggleMute}
              disabled={sending || composerDisabled}
              size={participantsBarSize}
              shape={participantsBarShape}
              gap={participantsBarGap}
              align={participantsBarAlign}
              directorMode={directorMode}
              directorBehavior={directorBehavior}
              selectedId={directorSelectedId}
              wiggleNonce={directorWiggleNonce}
              hintPosition={directorHintPosition}
              onSelectSpeaker={onSelectSpeaker}
            />
          </div>
        </div>
      )}

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
        {inlinePanel && <div className="border-b border-fg/10">{inlinePanel}</div>}
        <div className="relative flex items-end gap-2.5 p-2">
          {(onOpenPlusMenu || onAddAttachment) && (
            <button
              onClick={handlePlusClick}
              disabled={sending}
              className={cn(
                "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                radius.full,
                footerIconIdle,
                interactive.transition.fast,
                interactive.active.scale,
                "hover:bg-fg/10",
                footerIconHover,
                "disabled:cursor-not-allowed disabled:opacity-40",
              )}
              title={
                onOpenPlusMenu
                  ? t("groupChats.footer.moreOptions")
                  : t("groupChats.footer.addImage")
              }
              aria-label={
                onOpenPlusMenu
                  ? t("groupChats.footer.moreOptions")
                  : t("groupChats.footer.addImage")
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
                    "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                    radius.full,
                    "text-fg/60",
                    interactive.transition.fast,
                    interactive.active.scale,
                    "hover:bg-fg/10 hover:text-fg",
                    "disabled:cursor-not-allowed disabled:opacity-40",
                  )}
                  title={t("groupChats.footerExtra.cancelRecording")}
                  aria-label={t("groupChats.footerExtra.cancelRecording")}
                >
                  <X size={18} />
                </button>
              )}
              {recordingTranscribing ? (
                <div
                  className={cn(
                    "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                    radius.full,
                    "bg-accent text-black",
                  )}
                  aria-label={t("groupChats.footerExtra.transcribing")}
                  title={t("groupChats.footerExtra.transcribing")}
                >
                  <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" />
                </div>
              ) : (
                onMicClick && (
                  <button
                    onClick={onMicClick}
                    disabled={micDisabled}
                    className={cn(
                      "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                      radius.full,
                      "bg-accent text-black shadow-sm",
                      interactive.transition.fast,
                      interactive.active.scale,
                      "hover:brightness-110",
                      "disabled:cursor-not-allowed disabled:opacity-40",
                    )}
                    title={t("groupChats.footerExtra.stopAndTranscribe")}
                    aria-label={t("groupChats.footerExtra.stopAndTranscribe")}
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
                value={draft}
                onChange={(event) => handleDraftChange(event.target.value)}
                onKeyDown={handleKeyDown}
                data-tour-id="group-chat-composer"
                placeholder={
                  directorMode
                    ? t("groupChats.footer.directorPlaceholder")
                    : t("groupChats.footer.messagePlaceholder")
                }
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
                    "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                    radius.full,
                    footerIconIdle,
                    interactive.transition.fast,
                    interactive.active.scale,
                    "hover:bg-fg/10",
                    footerIconHover,
                    "disabled:cursor-not-allowed disabled:opacity-40",
                  )}
                  title={t("groupChats.footerExtra.recordVoice")}
                  aria-label={t("groupChats.footerExtra.recordVoice")}
                >
                  <Mic size={18} strokeWidth={2} />
                </button>
              )}

              <button
                onClick={handleSendClick}
                disabled={(sending && !onAbort) || composerDisabled}
                className={cn(
                  "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                  radius.full,
                  interactive.transition.fast,
                  interactive.active.scale,
                  sending && onAbort
                    ? "bg-red-400/90 text-white hover:brightness-110"
                    : canSend
                      ? "bg-accent text-black shadow-sm hover:brightness-110"
                      : hasFooterColor
                        ? "bg-fg/15 text-[var(--footer-fg-muted)] hover:bg-fg/20"
                        : "bg-fg/15 text-fg/55 hover:bg-fg/20",
                  "disabled:cursor-not-allowed disabled:opacity-40",
                )}
                title={sendButtonLabel}
                aria-label={sendButtonLabel}
              >
                {sending && onAbort ? (
                  <Square size={16} fill="currentColor" />
                ) : sending ? (
                  <span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
                ) : canSend ? (
                  <ArrowUp size={18} strokeWidth={2.75} />
                ) : onContinue ? (
                  <ChevronsRight size={18} />
                ) : (
                  <ArrowUp size={18} strokeWidth={2.75} />
                )}
              </button>
            </>
          )}
        </div>
      </div>
    </footer>
  );
}

// Mention picker item component
function MentionPickerItem({
  character,
  onClick,
  query,
}: {
  character: Character;
  onClick: () => void;
  query: string;
}) {
  const { t } = useI18n();
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  // Highlight matching text
  const highlightMatch = (text: string) => {
    if (!query) return text;
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    const index = lowerText.indexOf(lowerQuery);
    if (index === -1) return text;

    return (
      <>
        {text.slice(0, index)}
        <span className="text-accent font-medium">{text.slice(index, index + query.length)}</span>
        {text.slice(index + query.length)}
      </>
    );
  };

  const description = character.description || character.definition;

  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2.5 px-2 py-2 text-left",
        radius.md,
        "transition-colors",
        "hover:bg-fg/10 active:bg-fg/15",
      )}
    >
      <div
        className={cn(
          "h-8 w-8 shrink-0 rounded-full overflow-hidden",
          "bg-linear-to-br from-fg/8 to-fg/4",
          "ring-1 ring-fg/10",
        )}
      >
        {avatarUrl ? (
          <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-xs font-bold text-fg/60">
            {character.name.slice(0, 1).toUpperCase()}
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-fg truncate">{highlightMatch(character.name)}</p>
        {description && (
          <p className="text-[11px] text-fg/40 truncate">
            {description.slice(0, 50)}
            {description.length > 50 ? "..." : ""}
          </p>
        )}
      </div>
      <div className="text-[10px] text-fg/30 shrink-0">{t("groupChats.footer.tabToSelect")}</div>
    </button>
  );
}
