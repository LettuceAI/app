import { useEffect, useMemo, useRef } from "react";
import { ChevronsRight, Plus, SendHorizonal, Square, X } from "lucide-react";
import type { Character, ImageAttachment } from "../../../../core/storage/schemas";
import { radius, typography, interactive, shadows, cn } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { useI18n } from "../../../../core/i18n/context";

interface ChatFooterProps {
  draft: string;
  setDraft: (value: string) => void;
  error: string | null;
  sending: boolean;
  character: Character;
  onSendMessage: () => Promise<void>;
  onAbort?: () => Promise<void>;
  hasBackgroundImage?: boolean;
  footerOverlayClassName?: string;
  pendingAttachments?: ImageAttachment[];
  onAddAttachment?: (attachment: ImageAttachment) => void;
  onRemoveAttachment?: (attachmentId: string) => void;
  onOpenPlusMenu?: () => void;
  triggerFileInput?: boolean;
  onFileInputTriggered?: () => void;
  textareaRef?: React.RefObject<HTMLTextAreaElement | null>;
}

export function ChatFooter({
  draft,
  setDraft,
  error,
  sending,
  onSendMessage,
  onAbort,
  hasBackgroundImage,
  footerOverlayClassName: _footerOverlayClassName,
  pendingAttachments = [],
  onAddAttachment,
  onRemoveAttachment,
  onOpenPlusMenu,
  triggerFileInput,
  onFileInputTriggered,
  textareaRef: externalTextareaRef,
}: ChatFooterProps) {
  const { t } = useI18n();
  const hasDraft = draft.trim().length > 0;
  const hasAttachments = pendingAttachments.length > 0;
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

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
      if (!sending && (hasDraft || hasAttachments)) {
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

  return (
    <footer
      className={cn(
        "z-20 shrink-0 px-4 pb-6 pt-3",
        hasBackgroundImage ? "bg-transparent" : "bg-surface",
      )}
    >
      {error && (
        <div
          className={cn(
            "mb-3 px-4 py-2.5",
            radius.md,
            "border border-red-400/30 bg-red-400/10",
            typography.bodySmall.size,
            "text-red-200",
          )}
        >
          {error}
        </div>
      )}

      {/* Attachment Preview */}
      {hasAttachments && (
        <div className="mb-2 flex flex-wrap gap-2 overflow-visible p-1">
          {pendingAttachments.map((attachment) => (
            <div
              key={attachment.id}
              className={cn("relative", radius.md, "border border-fg/15 bg-fg/8")}
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
                  aria-label={t("chats.footer.removeAttachment")}
                >
                  <X className="h-5 w-5 text-fg drop-shadow-[0_1px_2px_rgba(0,0,0,0.25)]" />
                </button>
              )}
            </div>
          ))}
        </div>
      )}

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
          "border border-fg/15 bg-surface-el/65 backdrop-blur-md",
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
            title={onOpenPlusMenu ? t("chats.footer.moreOptions") : t("chats.footer.addImage")}
            aria-label={
              onOpenPlusMenu ? t("chats.footer.moreOptions") : t("chats.footer.addImageAttachment")
            }
          >
            <Plus size={20} />
          </button>
        )}

        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={" "}
          rows={1}
          className={cn(
            "peer max-h-32 flex-1 resize-none bg-transparent py-2.5",
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
            {t("chats.footer.sendMessagePlaceholder")}
          </span>
        )}
        <button
          onClick={sending && onAbort ? onAbort : onSendMessage}
          disabled={sending && !onAbort}
          className={cn(
            "mb-0.5 flex h-10 w-11 shrink-0 items-center justify-center self-end",
            radius.full,
            sending && onAbort
              ? "border border-red-400/40 bg-red-400/20 text-red-100"
              : hasDraft || hasAttachments
                ? "border border-emerald-400/40 bg-emerald-400/20 text-emerald-100"
                : "border border-fg/15 bg-fg/10 text-fg/70",
            interactive.transition.fast,
            interactive.active.scale,
            sending && onAbort && "hover:border-red-400/60 hover:bg-red-400/30",
            !sending &&
              (hasDraft || hasAttachments) &&
              "hover:border-emerald-400/60 hover:bg-emerald-400/30",
            !sending && !hasDraft && !hasAttachments && "hover:border-fg/25 hover:bg-fg/15",
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
            <Square size={18} fill="currentColor" />
          ) : sending ? (
            <span className="w-4 h-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
          ) : hasDraft || hasAttachments ? (
            <SendHorizonal size={18} />
          ) : (
            <ChevronsRight size={18} />
          )}
        </button>
      </div>
    </footer>
  );
}
