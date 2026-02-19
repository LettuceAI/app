import { useEffect, useRef, useMemo, useState } from "react";
import {
  Plus,
  SendHorizonal,
  X,
  Image as ImageIcon,
  Users,
  User,
  Square,
  RotateCcw,
} from "lucide-react";
import { radius, typography, interactive, shadows, cn } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { BottomMenu, MenuButton, MenuSection } from "../../../components";
import { ReferenceAvatar } from "./ReferenceSelector";
import type { Reference } from "./ReferenceSelector";

interface ImageAttachment {
  id: string;
  data: string;
  mimeType: string;
  filename?: string;
}

interface CreationHelperFooterProps {
  draft: string;
  setDraft: (value: string) => void;
  error: string | null;
  sending: boolean;
  onSendMessage: () => Promise<void>;
  onRetry?: () => void;
  onAbort?: () => void;
  pendingAttachments?: ImageAttachment[];
  onAddAttachment?: (attachment: ImageAttachment) => void;
  onRemoveAttachment?: (attachmentId: string) => void;
  references?: Reference[];
  onAddReference?: (ref: Reference) => void;
  onRemoveReference?: (refId: string) => void;
  onOpenReferenceSelector?: (type: "character" | "persona") => void;
}

export function CreationHelperFooter({
  draft,
  setDraft,
  error,
  sending,
  onSendMessage,
  onRetry,
  onAbort,
  pendingAttachments = [],
  onAddAttachment,
  onRemoveAttachment,
  references = [],
  onRemoveReference,
  onOpenReferenceSelector,
}: CreationHelperFooterProps) {
  const hasDraft = draft.trim().length > 0;
  const hasAttachments = pendingAttachments.length > 0;
  const hasReferences = references.length > 0;
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [showPlusMenu, setShowPlusMenu] = useState(false);

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
      if (!sending && (hasDraft || hasAttachments || hasReferences)) {
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
        const attachment: ImageAttachment = {
          id: crypto.randomUUID(),
          data: base64,
          mimeType: file.type,
          filename: file.name,
        };
        onAddAttachment(attachment);
      };
      reader.readAsDataURL(file);
    }

    event.target.value = "";
    setShowPlusMenu(false);
  };

  return (
    <footer className="z-20 shrink-0 px-4 pb-6 pt-3">
      {error && (
        <div
          className={cn(
            "mb-3 px-4 py-2.5 flex items-center justify-between gap-4",
            radius.md,
            "border border-danger/30 bg-danger/10",
            typography.bodySmall.size,
            "text-danger",
          )}
        >
          <span className="flex-1">{error}</span>
          {!sending && (
            <button
              onClick={() => (onRetry ? onRetry() : onSendMessage())}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1 rounded-full bg-danger/20 hover:bg-danger/30 text-fg/90 transition-colors whitespace-nowrap font-medium",
              )}
            >
              <RotateCcw className="h-3.5 w-3.5" />
              <span>Retry</span>
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

      {/* Reference Preview */}
      {hasReferences && (
        <div className="mb-2 flex flex-wrap gap-2 overflow-visible p-1">
          {references.map((ref) => (
            <div
              key={ref.id}
              className={cn(
                "flex items-center gap-2 px-2 py-1",
                radius.full,
                "border border-fg/20 bg-fg/10 text-fg/80 text-sm",
              )}
            >
              <ReferenceAvatar
                type={ref.type}
                id={ref.id}
                avatarPath={ref.avatarPath}
                avatarCrop={ref.avatarCrop}
                name={ref.name}
                size="sm"
              />
              <span>{ref.name}</span>
              {onRemoveReference && (
                <button
                  onClick={() => onRemoveReference(ref.id)}
                  className="ml-1 hover:text-fg"
                  aria-label="Remove reference"
                >
                  <X className="h-3.5 w-3.5" />
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
          "border border-fg/15 bg-fg/5 backdrop-blur-3xl",
          shadows.md,
        )}
      >
        {/* Plus button */}
        <button
          onClick={() => setShowPlusMenu(true)}
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
          title="More options"
          aria-label="More options"
        >
          <Plus size={20} />
        </button>

        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
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

        {draft.length === 0 && !hasAttachments && !hasReferences && (
          <span
            className={cn(
              "pointer-events-none absolute left-16",
              "top-1/2 -translate-y-1/2",
              "text-fg/40",
              "transition-opacity duration-150",
            )}
          >
            Describe your character...
          </span>
        )}
        <button
          onClick={() => {
            if (sending && onAbort) {
              onAbort();
            } else {
              onSendMessage();
            }
          }}
          disabled={!sending && !hasDraft && !hasAttachments && !hasReferences}
          className={cn(
            "mb-0.5 flex h-10 w-11 shrink-0 items-center justify-center self-end",
            radius.full,
            sending
              ? "border border-danger/40 bg-danger/20 text-danger"
              : hasDraft || hasAttachments || hasReferences
                ? "border border-accent/40 bg-accent/20 text-accent"
                : "border border-fg/15 bg-fg/10 text-fg/70",
            interactive.transition.fast,
            interactive.active.scale,
            sending && "hover:border-danger/60 hover:bg-danger/30",
            !sending &&
              (hasDraft || hasAttachments || hasReferences) &&
              "hover:border-accent/60 hover:bg-accent/30",
            !sending &&
              !hasDraft &&
              !hasAttachments &&
              !hasReferences &&
              "hover:border-fg/25 hover:bg-fg/15",
            "disabled:cursor-not-allowed disabled:opacity-40",
          )}
          title={sending ? "Stop generation" : "Send message"}
          aria-label={sending ? "Stop generation" : "Send message"}
        >
          {sending ? <Square className="h-4 w-4 fill-current" /> : <SendHorizonal size={18} />}
        </button>
      </div>

      {/* Plus Menu */}
      <BottomMenu
        isOpen={showPlusMenu}
        onClose={() => setShowPlusMenu(false)}
        title="Add to Message"
      >
        <MenuSection>
          <MenuButton
            icon={ImageIcon}
            title="Upload Image"
            description="Add an avatar or reference image"
            color="from-info to-info/80"
            onClick={() => {
              fileInputRef.current?.click();
            }}
          />
          {onOpenReferenceSelector && (
            <>
              <MenuButton
                icon={User}
                title="Reference Character"
                description="Use an existing character as inspiration"
                color="from-secondary to-danger/80"
                onClick={() => {
                  setShowPlusMenu(false);
                  onOpenReferenceSelector("character");
                }}
              />
              <MenuButton
                icon={Users}
                title="Reference Persona"
                description="Use your persona as context"
                color="from-warning to-warning/80"
                onClick={() => {
                  setShowPlusMenu(false);
                  onOpenReferenceSelector("persona");
                }}
              />
            </>
          )}
        </MenuSection>
      </BottomMenu>
    </footer>
  );
}
