import { memo, useState } from "react";
import { Archive, ArchiveRestore, Edit3, Plus, Trash2 } from "lucide-react";

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

export function SessionCard({
  session,
  characters,
  onSelect,
  onDelete,
  onRename,
  onArchive,
  onUnarchive,
  onDuplicate,
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
  isBusy: boolean;
  isArchived?: boolean;
}) {
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

  const characterNames = session.characterIds
    .map((id) => characters.find((c) => c.id === id)?.name || "Unknown")
    .slice(0, 3)
    .join(", ");

  const extraCount = session.characterIds.length - 3;
  const characterSummary =
    extraCount > 0 ? `${characterNames} +${extraCount} more` : characterNames;

  return (
    <div
      className={cn(
        "border border-fg/10 bg-fg/5 overflow-hidden",
        radius.lg,
        isArchived && "border-warning/20 bg-warning/5",
      )}
    >
      <button
        onClick={onSelect}
        disabled={isBusy || isRenaming}
        className="w-full p-4 text-left disabled:opacity-50 active:bg-fg/10 transition-colors"
      >
        <div className="flex items-start gap-3">
          <div className="relative h-12 w-14 shrink-0">
            {avatarCharacters.map((char, index) => (
              <div
                key={char.id}
                className={cn(
                  "absolute h-8 w-8 overflow-hidden rounded-full",
                  "border-2 border-[#0c0d13]",
                  "bg-linear-to-br from-white/10 to-white/5",
                )}
                style={{
                  left: `${index * 10}px`,
                  zIndex: 3 - index,
                }}
              >
                <CharacterMiniAvatar character={char} />
              </div>
            ))}
            {session.characterIds.length > 3 && (
              <div
                className={cn(
                  "absolute h-8 w-8 overflow-hidden rounded-full",
                  "border-2 border-[#0c0d13]",
                  "bg-fg/10",
                  "flex items-center justify-center",
                )}
                style={{
                  left: `${3 * 10}px`,
                  zIndex: 0,
                }}
              >
                <span className="text-[10px] font-medium text-fg/60">
                  +{session.characterIds.length - 3}
                </span>
              </div>
            )}
          </div>

          <div className="flex-1 min-w-0">
            <h3
              className={cn(typography.h3.size, typography.h3.weight, "text-fg mb-1 truncate")}
            >
              {session.name}
            </h3>

            <p className={cn(typography.caption.size, "text-fg/40 truncate mb-1")}>
              {characterSummary}
            </p>

            <p className={cn(typography.bodySmall.size, "text-fg/50 mb-2")}>
              {formatTimeAgo(session.updatedAt)} • {session.messageCount}{" "}
              {session.messageCount === 1 ? "message" : "messages"}
            </p>

            {session.lastMessage && (
              <p
                className={cn(
                  typography.bodySmall.size,
                  "text-fg/70 line-clamp-2 leading-relaxed",
                )}
              >
                {session.lastMessage}
              </p>
            )}
          </div>
        </div>
      </button>

      {isRenaming && (
        <div className="px-4 pb-3 border-t border-fg/5 pt-3">
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
              typography.body.size,
              "focus:outline-none focus:border-info/60",
            )}
            placeholder="Chat title..."
          />
          <div className="flex gap-2">
            <button
              onClick={handleRenameSubmit}
              disabled={!editTitle.trim()}
              className={cn(
                "flex-1 px-3 py-2 border border-info/40 bg-info/20 text-info",
                radius.md,
                typography.bodySmall.size,
                "active:scale-95 disabled:opacity-50 transition-all",
              )}
            >
              Save
            </button>
            <button
              onClick={handleCancel}
              className={cn(
                "flex-1 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
                radius.md,
                typography.bodySmall.size,
                "active:scale-95 transition-all",
              )}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {!isRenaming && (
        <div className="px-4 pb-3 border-t border-fg/5 pt-3 flex flex-wrap gap-2">
          <button
            onClick={onDuplicate}
            disabled={isBusy}
            className={cn(
              "flex items-center gap-2 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
              radius.md,
              typography.bodySmall.size,
              "active:scale-95 active:bg-accent/10 active:text-accent/80 active:border-accent/40 disabled:opacity-50 transition-all",
            )}
          >
            <Plus size={14} />
            New Chat
          </button>
          <button
            onClick={() => setIsRenaming(true)}
            disabled={isBusy}
            className={cn(
              "flex items-center gap-2 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
              radius.md,
              typography.bodySmall.size,
              "active:scale-95 active:bg-info/10 active:text-info active:border-info/40 disabled:opacity-50 transition-all",
            )}
          >
            <Edit3 size={14} />
            Rename
          </button>
          {isArchived ? (
            <button
              onClick={onUnarchive}
              disabled={isBusy}
              className={cn(
                "flex items-center gap-2 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
                radius.md,
                typography.bodySmall.size,
                "active:scale-95 active:bg-warning/10 active:text-warning active:border-warning/40 disabled:opacity-50 transition-all",
              )}
            >
              <ArchiveRestore size={14} />
              Unarchive
            </button>
          ) : (
            <button
              onClick={onArchive}
              disabled={isBusy}
              className={cn(
                "flex items-center gap-2 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
                radius.md,
                typography.bodySmall.size,
                "active:scale-95 active:bg-warning/10 active:text-warning active:border-warning/40 disabled:opacity-50 transition-all",
              )}
            >
              <Archive size={14} />
              Archive
            </button>
          )}
          <button
            onClick={onDelete}
            disabled={isBusy}
            className={cn(
              "flex items-center gap-2 px-3 py-2 border border-fg/10 bg-fg/5 text-fg/60",
              radius.md,
              typography.bodySmall.size,
              "active:scale-95 active:bg-danger/10 active:text-danger active:border-danger/40 disabled:opacity-50 transition-all",
            )}
          >
            <Trash2 size={14} />
            Delete
          </button>
        </div>
      )}
    </div>
  );
}
