import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { ChevronsRight, Plus, SendHorizonal, Square, X } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import type { Character, ImageAttachment, Persona } from "../../../../core/storage/schemas";
import { radius, typography, interactive, shadows, cn } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { useAvatar } from "../../../hooks/useAvatar";
import { AvatarImage } from "../../../components/AvatarImage";

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
  pendingAttachments?: ImageAttachment[];
  onAddAttachment?: (attachment: ImageAttachment) => void;
  onRemoveAttachment?: (attachmentId: string) => void;
  onOpenPlusMenu?: () => void;
  triggerFileInput?: boolean;
  onFileInputTriggered?: () => void;
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
  pendingAttachments = [],
  onAddAttachment,
  onRemoveAttachment,
  onOpenPlusMenu,
  triggerFileInput,
  onFileInputTriggered,
}: GroupChatFooterProps) {
  const hasDraft = draft.trim().length > 0;
  const hasAttachments = pendingAttachments.length > 0;
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
    [setDraft],
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
      if (!sending && (hasDraft || hasAttachments)) {
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
    } else if (hasDraft || hasAttachments) {
      onSendMessage();
    } else if (onContinue) {
      onContinue();
    }
  };

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
        "z-20 shrink-0 px-4 pb-3 pt-3",
        hasBackgroundImage ? footerOverlayClassName || "bg-surface/85" : "bg-surface",
      )}
    >
      {error && (
        <div
          className={cn(
            "mb-3 px-4 py-2.5 flex items-start justify-between gap-2",
            radius.md,
            "border border-danger/30 bg-danger/10",
            typography.bodySmall.size,
            "text-danger",
          )}
        >
          <span className="flex-1">{error}</span>
          {setError && (
            <button
              onClick={() => setError(null)}
              className={cn(
                "shrink-0 p-1 rounded",
                "text-danger/70 hover:text-danger hover:bg-danger/20",
                interactive.transition.fast,
              )}
              aria-label="Dismiss error"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
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
                  aria-label="Remove attachment"
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
                Mention a character
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
                <div className="px-3 py-4 text-center text-sm text-fg/40">No characters found</div>
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

      <div
        className={cn(
          "relative flex items-end gap-2.5 p-2",
          "rounded-4xl",
          "border border-fg/15 bg-fg/5 backdrop-blur-md",
          shadows.md,
        )}
      >
        {/* Plus button */}
        {(onOpenPlusMenu || onAddAttachment) && (
          <button
            onClick={handlePlusClick}
            disabled={sending}
            className={cn(
              "mb-0.5 flex h-10 w-11 shrink-0 items-center justify-center self-end",
              radius.full,
              "border border-fg/15 bg-fg/10 text-fg/70",
              interactive.transition.fast,
              interactive.active.scale,
              "hover:border-fg/25 hover:bg-fg/15",
              "disabled:cursor-not-allowed disabled:opacity-40",
            )}
            title={onOpenPlusMenu ? "More options" : "Add image"}
            aria-label={onOpenPlusMenu ? "More options" : "Add image attachment"}
          >
            <Plus size={20} />
          </button>
        )}

        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(event) => handleDraftChange(event.target.value)}
          onKeyDown={handleKeyDown}
          placeholder=" "
          rows={1}
          className={cn(
            "max-h-32 flex-1 resize-none bg-transparent py-2.5",
            typography.body.size,
            "text-fg placeholder:text-transparent",
            "focus:outline-none",
          )}
          disabled={sending}
        />

        {draft.length === 0 && !hasAttachments && (
          <span
            className={cn(
              "pointer-events-none absolute",
              onOpenPlusMenu || onAddAttachment ? "left-16" : "left-5",
              "top-1/2 -translate-y-1/2",
              "text-fg/40",
              "transition-opacity duration-150",
              "peer-not-placeholder-shown:opacity-0",
              "peer-focus:opacity-70",
            )}
          >
            Message... (@ to mention)
          </span>
        )}

        <button
          onClick={handleSendClick}
          disabled={sending && !onAbort}
          className={cn(
            "mb-0.5 flex h-10 w-11 shrink-0 items-center justify-center self-end",
            radius.full,
            sending && onAbort
              ? "border border-danger/40 bg-danger/20 text-danger"
              : hasDraft || hasAttachments
                ? "border border-accent/40 bg-accent/20 text-accent"
                : onContinue
                  ? "border border-fg/15 bg-fg/10 text-fg/70"
                  : "border border-fg/15 bg-fg/10 text-fg/40",
            interactive.transition.fast,
            interactive.active.scale,
            sending && onAbort && "hover:border-danger/60 hover:bg-danger/30",
            !sending && (hasDraft || hasAttachments) && "hover:border-accent/60 hover:bg-accent/30",
            !sending &&
              !hasDraft &&
              !hasAttachments &&
              onContinue &&
              "hover:border-fg/25 hover:bg-fg/15",
            "disabled:cursor-not-allowed disabled:opacity-40",
          )}
          title={
            sending && onAbort
              ? "Stop generation"
              : hasDraft || hasAttachments
                ? "Send message"
                : onContinue
                  ? "Continue conversation"
                  : "Send message"
          }
          aria-label={
            sending && onAbort
              ? "Stop generation"
              : hasDraft || hasAttachments
                ? "Send message"
                : onContinue
                  ? "Continue conversation"
                  : "Send message"
          }
        >
          {sending && onAbort ? (
            <Square size={18} fill="currentColor" />
          ) : sending ? (
            <span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
          ) : hasDraft || hasAttachments ? (
            <SendHorizonal size={18} />
          ) : onContinue ? (
            <ChevronsRight size={18} />
          ) : (
            <SendHorizonal size={18} />
          )}
        </button>
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
          "bg-linear-to-br from-white/10 to-white/5",
          "ring-1 ring-white/10",
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
      <div className="text-[10px] text-fg/30 shrink-0">Tab to select</div>
    </button>
  );
}
