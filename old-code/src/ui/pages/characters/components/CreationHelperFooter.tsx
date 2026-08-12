import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowUp,
  Image as ImageIcon,
  Plus,
  RotateCcw,
  Square,
  User,
  Users,
  X,
} from "lucide-react";
import { radius, typography, interactive, shadows, cn } from "../../../design-tokens";
import { getPlatform } from "../../../../core/utils/platform";
import { BottomMenu, MenuButton, MenuSection } from "../../../components";
import { ReferenceAvatar } from "./ReferenceSelector";
import type { Reference } from "./ReferenceSelector";
import { useI18n } from "../../../../core/i18n/context";

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
  const { t } = useI18n();
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
        void onSendMessage();
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

  const handlePlusClick = () => {
    setShowPlusMenu(true);
  };

  const handleSendButtonClick = () => {
    if (sending && onAbort) {
      onAbort();
      return;
    }
    void onSendMessage();
  };

  return (
    <>
      <footer className="z-20 shrink-0 px-4 pb-6 pt-3 bg-surface">
        {error && (
          <div
            className={cn(
              "mb-3 px-4 py-2.5 flex items-center justify-between gap-4",
              radius.md,
              "border border-red-400/30 bg-red-400/10",
              typography.bodySmall.size,
              "text-red-200",
            )}
          >
            <span className="flex-1">{error}</span>
            {!sending && (
              <button
                onClick={() => (onRetry ? onRetry() : onSendMessage())}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1 rounded-full",
                  "bg-red-400/20 hover:bg-red-400/30 text-fg/90 font-medium",
                  "transition-colors whitespace-nowrap",
                )}
              >
                <RotateCcw className="h-3.5 w-3.5" />
                <span>{t("characters.creationHelper.retry")}</span>
              </button>
            )}
          </div>
        )}

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
                    aria-label={t("characters.creationHelper.removeAttachment")}
                  >
                    <X className="h-5 w-5 text-fg drop-shadow-[0_1px_2px_rgba(0,0,0,0.25)]" />
                  </button>
                )}
              </div>
            ))}
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

        <div
          className={cn(
            "relative",
            "rounded-4xl",
            "border border-fg/15 bg-surface-el/65 backdrop-blur-md",
            shadows.md,
          )}
        >
          {hasReferences && (
            <div className="border-b border-fg/10 px-3 py-2">
              <div className="flex flex-wrap gap-1.5">
                {references.map((ref) => (
                  <div
                    key={ref.id}
                    className={cn(
                      "flex items-center gap-1.5 px-2 py-0.5",
                      radius.full,
                      "border border-fg/15 bg-fg/8 text-fg/85 text-xs",
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
                    <span className="max-w-[140px] truncate">{ref.name}</span>
                    {onRemoveReference && (
                      <button
                        onClick={() => onRemoveReference(ref.id)}
                        className="text-fg/50 hover:text-fg"
                        aria-label={t("characters.creationHelper.removeReference")}
                      >
                        <X className="h-3 w-3" />
                      </button>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="relative flex items-end gap-2.5 p-2">
            <button
              onClick={handlePlusClick}
              disabled={sending}
              className={cn(
                "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                radius.full,
                "text-fg/60",
                interactive.transition.fast,
                interactive.active.scale,
                "hover:bg-fg/10 hover:text-fg",
                "disabled:cursor-not-allowed disabled:opacity-40",
              )}
              title={t("characters.creationHelper.moreOptions")}
              aria-label={t("characters.creationHelper.moreOptions")}
            >
              <Plus size={20} />
            </button>

            <textarea
              ref={textareaRef}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t("characters.creationHelper.describePlaceholder")}
              rows={1}
              className={cn(
                "max-h-32 flex-1 resize-none bg-transparent py-2.5",
                typography.body.size,
                "text-fg placeholder:text-fg/40",
                "focus:outline-none",
              )}
              disabled={sending}
            />

            <button
              onClick={handleSendButtonClick}
              disabled={sending && !onAbort}
              className={cn(
                "mb-0.5 flex h-[43px] w-[43px] shrink-0 items-center justify-center self-end",
                radius.full,
                interactive.transition.fast,
                interactive.active.scale,
                sending && onAbort
                  ? "bg-red-400/90 text-white hover:brightness-110"
                  : hasDraft || hasAttachments || hasReferences
                    ? "bg-accent text-black shadow-sm hover:brightness-110"
                    : "bg-fg/15 text-fg/55 hover:bg-fg/20",
                "disabled:cursor-not-allowed disabled:opacity-40",
              )}
              title={
                sending && onAbort
                  ? t("characters.creationHelper.stopGeneration")
                  : t("characters.creationHelper.sendMessage")
              }
              aria-label={
                sending && onAbort
                  ? t("characters.creationHelper.stopGeneration")
                  : t("characters.creationHelper.sendMessage")
              }
            >
              {sending && onAbort ? (
                <Square size={16} fill="currentColor" />
              ) : sending ? (
                <span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
              ) : (
                <ArrowUp size={18} strokeWidth={2.75} />
              )}
            </button>
          </div>
        </div>
      </footer>

      <BottomMenu
        isOpen={showPlusMenu}
        onClose={() => setShowPlusMenu(false)}
        title={t("characters.creationHelper.moreOptions")}
      >
        <MenuSection>
          <MenuButton
            icon={ImageIcon}
            title={t("characters.creationHelper.uploadImageTitle")}
            description={t("characters.creationHelper.uploadImageDesc")}
            color="from-info to-info/80"
            onClick={() => {
              fileInputRef.current?.click();
            }}
          />
          {onOpenReferenceSelector && (
            <>
              <MenuButton
                icon={User}
                title={t("characters.creationHelper.referenceCharacterTitle")}
                description={t("characters.creationHelper.referenceCharacterDesc")}
                color="from-secondary to-danger/80"
                onClick={() => {
                  setShowPlusMenu(false);
                  onOpenReferenceSelector("character");
                }}
              />
              <MenuButton
                icon={Users}
                title={t("characters.creationHelper.referencePersonaTitle")}
                description={t("characters.creationHelper.referencePersonaDesc")}
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
    </>
  );
}

