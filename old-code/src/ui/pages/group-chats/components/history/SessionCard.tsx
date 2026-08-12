import { memo, useState } from "react";
import { Archive, ArchiveRestore, Copy, Download, Edit3, Trash2 } from "lucide-react";

import { useI18n } from "../../../../../core/i18n/context";
import type { GroupSessionPreview, Character } from "../../../../../core/storage/schemas";
import { AvatarImage } from "../../../../components/AvatarImage";
import { useAvatar } from "../../../../hooks/useAvatar";
import { typography, radius, cn } from "../../../../design-tokens";
import { formatTimeAgo } from "../../utils/formatTimeAgo";

const CharacterMiniAvatar = memo(({ character }: { character: Character }) => {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  if (avatarUrl) {
    return (
      <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
    );
  }

  const initials = character.name.slice(0, 2).toUpperCase();
  return (
    <div className="flex h-full w-full items-center justify-center text-[10px] font-bold text-fg/60">
      {initials}
    </div>
  );
});

CharacterMiniAvatar.displayName = "CharacterMiniAvatar";

function IconAction({
  icon: Icon,
  label,
  onClick,
  disabled,
  danger = false,
}: {
  icon: typeof Copy;
  label: string;
  onClick: () => void;
  disabled: boolean;
  danger?: boolean;
}) {
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      disabled={disabled}
      title={label}
      aria-label={label}
      className={cn(
        "rounded-lg p-2 transition-colors",
        danger
          ? "text-fg/35 hover:bg-danger/10 hover:text-danger"
          : "text-fg/45 hover:bg-fg/6 hover:text-fg/75",
        disabled && "opacity-50",
      )}
    >
      <Icon size={14} />
    </button>
  );
}

export function SessionCard({
  session,
  characters,
  onSelect,
  onDelete,
  onRename,
  onArchive,
  onUnarchive,
  onDuplicate,
  onExport,
  isBusy,
  isArchived = false,
}: {
  session: GroupSessionPreview;
  characters: Character[];
  onSelect: () => void;
  onDelete: () => void;
  onRename: (newTitle: string) => void;
  onArchive?: () => void;
  onUnarchive?: () => void;
  onDuplicate: () => void;
  onExport?: () => void;
  isBusy: boolean;
  isArchived?: boolean;
}) {
  const { t } = useI18n();
  const [isRenaming, setIsRenaming] = useState(false);
  const [editTitle, setEditTitle] = useState(session.name);

  const handleRenameSubmit = () => {
    if (editTitle.trim() && editTitle !== session.name) {
      onRename(editTitle.trim());
    }
    setIsRenaming(false);
  };

  const handleCancel = () => {
    setEditTitle(session.name);
    setIsRenaming(false);
  };

  const avatarCharacters = session.characterIds
    .slice(0, 3)
    .map((id) => characters.find((c) => c.id === id))
    .filter(Boolean) as Character[];

  return (
    <div
      className={cn(
        "group overflow-hidden border transition-colors",
        radius.lg,
        "border-fg/8 bg-fg/3 hover:border-fg/12 hover:bg-fg/4",
        isArchived && "border-amber-400/20 bg-amber-400/5",
      )}
    >
      <button
        onClick={onSelect}
        disabled={isBusy || isRenaming}
        className="w-full px-4 py-3 text-left disabled:opacity-50 transition-colors"
      >
        <div className="flex items-start gap-3">
          <div className="flex shrink-0 -space-x-2 pt-0.5">
            {avatarCharacters.map((char) => (
              <div
                key={char.id}
                className={cn(
                  "h-7 w-7 overflow-hidden rounded-full",
                  "ring-2 ring-surface",
                  "bg-linear-to-br from-fg/8 to-fg/4",
                )}
              >
                <CharacterMiniAvatar character={char} />
              </div>
            ))}
            {session.characterIds.length > 3 && (
              <div
                className={cn(
                  "flex h-7 w-7 items-center justify-center rounded-full",
                  "ring-2 ring-surface bg-fg/10",
                  "text-[10px] font-semibold text-fg/60",
                )}
              >
                +{session.characterIds.length - 3}
              </div>
            )}
          </div>

          <div className="min-w-0 flex-1">
            <div className="mb-1 flex items-center gap-2">
              <h3 className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/92")}>
                {session.name}
              </h3>
              {isArchived ? (
                <span className="inline-flex shrink-0 items-center rounded-md border border-amber-400/25 bg-amber-400/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-200/80">
                  {t("chats.history.archivedBadge")}
                </span>
              ) : null}
            </div>
            {session.lastMessage ? (
              <p
                className={cn(typography.bodySmall.size, "line-clamp-2 leading-relaxed text-fg/58")}
              >
                {session.lastMessage}
              </p>
            ) : (
              <p className={cn(typography.bodySmall.size, "text-fg/32")}>
                {t("groupChats.list.emptyChatPreview")}
              </p>
            )}
          </div>
          <span className={cn(typography.caption.size, "shrink-0 pt-0.5 text-fg/35")}>
            {formatTimeAgo(session.updatedAt)}
          </span>
        </div>
      </button>

      {isRenaming ? (
        <div className="border-t border-fg/8 px-4 pb-3 pt-3">
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleRenameSubmit();
              if (e.key === "Escape") handleCancel();
            }}
            autoFocus
            className={cn(
              "w-full px-3 py-2 bg-fg/10 border border-fg/20 text-fg mb-2",
              radius.md,
              typography.bodySmall.size,
              "focus:outline-none focus:border-info/60",
            )}
            placeholder={t("groupChats.session.chatTitlePlaceholder")}
          />
          <div className="flex gap-2">
            <button
              onClick={handleRenameSubmit}
              disabled={!editTitle.trim()}
              className={cn(
                "flex-1 px-3 py-2 border border-info/40 bg-info/20 text-info",
                radius.md,
                typography.caption.size,
                "active:scale-95 disabled:opacity-50 transition-all",
              )}
            >
              {t("common.buttons.save")}
            </button>
            <button
              onClick={handleCancel}
              className={cn(
                "flex-1 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
                radius.md,
                typography.caption.size,
                "active:scale-95 transition-all",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
          </div>
        </div>
      ) : (
        <div className="flex items-center justify-between gap-3 border-t border-fg/8 px-3 py-1.5">
          <span className={cn(typography.caption.size, "text-fg/38")}>
            {t("groupChats.session.messageCount", {
              count: session.messageCount.toLocaleString(),
            })}
          </span>
          <div className="flex items-center gap-1">
            <IconAction
              icon={Copy}
              label={t("groupChats.session.duplicate")}
              onClick={onDuplicate}
              disabled={isBusy}
            />
            {onExport && (
              <IconAction
                icon={Download}
                label={t("groupChats.session.export")}
                onClick={onExport}
                disabled={isBusy}
              />
            )}
            <IconAction
              icon={Edit3}
              label={t("groupChats.session.rename")}
              onClick={() => setIsRenaming(true)}
              disabled={isBusy}
            />
            {isArchived
              ? onUnarchive && (
                  <IconAction
                    icon={ArchiveRestore}
                    label={t("groupChats.session.unarchive")}
                    onClick={onUnarchive}
                    disabled={isBusy}
                  />
                )
              : onArchive && (
                  <IconAction
                    icon={Archive}
                    label={t("groupChats.session.archive")}
                    onClick={onArchive}
                    disabled={isBusy}
                  />
                )}
            <IconAction
              icon={Trash2}
              label={t("common.buttons.delete")}
              onClick={onDelete}
              disabled={isBusy}
              danger
            />
          </div>
        </div>
      )}
    </div>
  );
}
