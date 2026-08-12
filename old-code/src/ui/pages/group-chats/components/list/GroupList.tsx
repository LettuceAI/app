import { useEffect, useRef, useState, memo } from "react";
import { BookOpen, ChevronRight, Loader2, Rocket, Users } from "lucide-react";

import { useI18n } from "../../../../../core/i18n/context";
import type {
  GroupPreview,
  GroupChatsViewMode,
  Character,
} from "../../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, cn } from "../../../../design-tokens";
import { useAvatar } from "../../../../hooks/useAvatar";
import { useRocketEasterEgg } from "../../../../hooks/useRocketEasterEgg";
import { AvatarImage } from "../../../../components/AvatarImage";
import { formatTimeAgo } from "../../utils/formatTimeAgo";
import { isRenderableImageUrl } from "../../../../../core/utils/image";
import { openDocs } from "../../../../../core/utils/docs";

export function GroupList({
  groups,
  characters,
  viewMode,
  openingGroupId,
  onOpenGroup,
  onLongPress,
}: {
  groups: GroupPreview[];
  characters: Character[];
  viewMode: GroupChatsViewMode;
  openingGroupId: string | null;
  onOpenGroup: (group: GroupPreview) => void;
  onLongPress: (group: GroupPreview) => void;
}) {
  const [visibleCount, setVisibleCount] = useState(10);

  useEffect(() => {
    if (visibleCount < groups.length) {
      const timer = setTimeout(() => {
        setVisibleCount((prev) => Math.min(prev + 10, groups.length));
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [visibleCount, groups.length]);

  useEffect(() => {
    setVisibleCount(10);
  }, [groups]);

  return (
    <div className="space-y-2 lg:space-y-3 pb-24">
      {groups.slice(0, visibleCount).map((group) => (
        <GroupCard
          key={group.id}
          group={group}
          characters={characters}
          viewMode={viewMode}
          isOpening={openingGroupId === group.id}
          disabled={openingGroupId !== null}
          onOpen={onOpenGroup}
          onLongPress={onLongPress}
        />
      ))}
    </div>
  );
}

export function GroupListSkeleton() {
  return (
    <div className={spacing.item}>
      {[0, 1, 2].map((index) => (
        <div
          key={index}
          className={cn("h-20 animate-pulse p-4", "rounded-2xl", "border border-fg/5 bg-fg/5")}
        >
          <div className="flex items-center gap-3">
            <div className="h-12 w-12 rounded-full bg-fg/10" />
            <div className="flex-1 space-y-2">
              <div className="h-4 w-1/3 rounded-full bg-fg/10" />
              <div className="h-3 w-2/3 rounded-full bg-fg/5" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

export function EmptyState() {
  const { t } = useI18n();
  const rocket = useRocketEasterEgg();
  return (
    <div
      className={cn(
        "relative p-8 text-center overflow-hidden",
        radius.lg,
        "border border-dashed border-fg/10 bg-fg/2",
      )}
      {...rocket.bind}
    >
      {rocket.isLaunched && (
        <div className="pointer-events-none absolute bottom-4 left-1/2 -translate-x-1/2 rocket-launch">
          <div className="flex h-9 w-9 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
            <Rocket className="h-4 w-4 text-fg/80" />
          </div>
        </div>
      )}
      <div className={spacing.field}>
        <Users className="mx-auto h-12 w-12 text-fg/30 mb-4" />
        <h3 className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>
          {t("groupChats.list.noGroupChatsYet")}
        </h3>
        <p className={cn(typography.body.size, typography.body.lineHeight, "text-fg/50")}>
          {t("groupChats.list.noGroupChatsDesc")}
        </p>
        <button
          onClick={() => void openDocs("groupChats")}
          className="mt-4 inline-flex items-center gap-1.5 text-xs font-medium text-fg/40 transition active:scale-95 hover:text-fg/70"
        >
          <BookOpen className="h-3.5 w-3.5" />
          {t("common.buttons.learnMore")}
        </button>
      </div>
    </div>
  );
}

function isImageLike(s?: string) {
  return isRenderableImageUrl(s);
}

const CharacterMiniAvatar = memo(({ character }: { character: Character }) => {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  if (avatarUrl && isImageLike(avatarUrl)) {
    return (
      <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
    );
  }

  const initials = character.name.slice(0, 2).toUpperCase();
  return (
    <div className="flex h-full w-full items-center justify-center text-xs font-bold text-fg/60">
      {initials}
    </div>
  );
});

CharacterMiniAvatar.displayName = "CharacterMiniAvatar";

function AvatarStack({
  group,
  characters,
  size,
}: {
  group: GroupPreview;
  characters: Character[];
  size: "md" | "lg";
}) {
  const avatarCharacters = group.characterIds
    .slice(0, 3)
    .map((id) => characters.find((c) => c.id === id))
    .filter(Boolean) as Character[];
  const sizeClass = size === "lg" ? "h-10 w-10" : "h-8 w-8";
  const ringClass =
    size === "lg" ? "border border-fg/10" : "ring-2 ring-surface";

  return (
    <div className={cn("flex shrink-0 -space-x-2", size === "md" && "pt-0.5")}>
      {avatarCharacters.map((character) => (
        <div
          key={character.id}
          className={cn(
            "flex items-center justify-center overflow-hidden rounded-full",
            sizeClass,
            ringClass,
            "bg-linear-to-br from-fg/8 to-fg/4",
          )}
        >
          <CharacterMiniAvatar character={character} />
        </div>
      ))}
      {group.characterIds.length > 3 && (
        <div
          className={cn(
            "flex items-center justify-center rounded-full",
            sizeClass,
            ringClass,
            "bg-fg/10 text-[10px] font-semibold text-fg/60",
          )}
        >
          +{group.characterIds.length - 3}
        </div>
      )}
    </div>
  );
}

const GroupCard = memo(
  ({
    group,
    characters,
    viewMode,
    isOpening,
    disabled,
    onOpen,
    onLongPress,
  }: {
    group: GroupPreview;
    characters: Character[];
    viewMode: GroupChatsViewMode;
    isOpening: boolean;
    disabled: boolean;
    onOpen: (group: GroupPreview) => void;
    onLongPress: (group: GroupPreview) => void;
  }) => {
    const { t } = useI18n();
    const longPressTimer = useRef<number | null>(null);
    const isLongPress = useRef(false);

    const hasSession = Boolean(group.latestSessionId);
    const recency = group.latestSessionUpdatedAt ?? group.updatedAt;
    const preview = group.latestSessionMessage?.trim() || null;

    const handlePointerDown = () => {
      isLongPress.current = false;
      longPressTimer.current = window.setTimeout(() => {
        isLongPress.current = true;
        onLongPress(group);
      }, 500);
    };

    const handlePointerUp = () => {
      if (longPressTimer.current) {
        clearTimeout(longPressTimer.current);
        longPressTimer.current = null;
      }
    };

    const handlePointerLeave = () => {
      if (longPressTimer.current) {
        clearTimeout(longPressTimer.current);
        longPressTimer.current = null;
      }
    };

    const handleClick = () => {
      if (isLongPress.current) {
        isLongPress.current = false;
        return;
      }
      onOpen(group);
    };

    const handleContextMenu = (e: React.MouseEvent) => {
      e.preventDefault();
      onLongPress(group);
    };

    const pressHandlers = {
      onClick: handleClick,
      onContextMenu: handleContextMenu,
      onPointerDown: handlePointerDown,
      onPointerUp: handlePointerUp,
      onPointerLeave: handlePointerLeave,
    };

    const previewNode = isOpening ? (
      <span className={cn(typography.caption.size, "truncate text-fg/50")}>
        {t("groupChats.list.startingChat")}
      </span>
    ) : preview ? (
      <span className={cn(typography.caption.size, "truncate text-fg/50")}>{preview}</span>
    ) : (
      <span className={cn(typography.caption.size, "truncate text-fg/32")}>
        {hasSession ? t("groupChats.list.emptyChatPreview") : t("groupChats.list.noChatsYet")}
      </span>
    );

    if (viewMode === "detailed") {
      const castNames = group.characterIds
        .slice(0, 3)
        .map((id) => characters.find((c) => c.id === id)?.name)
        .filter(Boolean)
        .join(", ");
      const castExtra = group.characterIds.length > 3 ? ` +${group.characterIds.length - 3}` : "";

      return (
        <button
          {...pressHandlers}
          disabled={disabled && !isOpening}
          aria-busy={isOpening}
          className={cn(
            "group w-full overflow-hidden p-0 text-left",
            radius.lg,
            "border transition-colors",
            "border-fg/8 bg-fg/3 hover:border-fg/12 hover:bg-fg/4",
            disabled && !isOpening && "opacity-60",
          )}
        >
          <span className="flex items-start gap-3 px-4 py-3">
            <AvatarStack group={group} characters={characters} size="md" />
            <span className="min-w-0 flex-1">
              <span
                className={cn(
                  typography.bodySmall.size,
                  "block truncate font-semibold text-fg/92",
                )}
              >
                {group.name}
              </span>
              {isOpening ? (
                <span className={cn(typography.bodySmall.size, "mt-1 block text-fg/58")}>
                  {t("groupChats.list.startingChat")}
                </span>
              ) : preview ? (
                <span
                  className={cn(
                    typography.bodySmall.size,
                    "mt-1 line-clamp-2 leading-relaxed text-fg/58",
                  )}
                >
                  {preview}
                </span>
              ) : (
                <span className={cn(typography.bodySmall.size, "mt-1 block text-fg/32")}>
                  {hasSession
                    ? t("groupChats.list.emptyChatPreview")
                    : t("groupChats.list.noChatsYet")}
                </span>
              )}
            </span>
            <span className="shrink-0 pt-0.5">
              {isOpening ? (
                <Loader2 className="h-4 w-4 animate-spin text-fg/40" />
              ) : (
                hasSession && (
                  <span className={cn(typography.caption.size, "text-fg/35")}>
                    {formatTimeAgo(recency)}
                  </span>
                )
              )}
            </span>
          </span>
          <span className="flex items-center justify-between gap-3 border-t border-fg/8 px-4 py-1.5">
            <span className={cn(typography.caption.size, "truncate text-fg/38")}>
              {hasSession
                ? `${t("groupChats.list.sessionCount", { count: String(group.sessionCount) })} · ${t(
                    "groupChats.session.messageCount",
                    { count: String(group.latestSessionMessageCount) },
                  )}`
                : t("groupChats.list.startFirstChatHint")}
            </span>
            <span className={cn(typography.caption.size, "shrink-0 truncate text-fg/30")}>
              {castNames}
              {castExtra}
            </span>
          </span>
        </button>
      );
    }

    return (
      <button
        {...pressHandlers}
        disabled={disabled && !isOpening}
        aria-busy={isOpening}
        className={cn(
          "group relative flex w-full items-center gap-3.5 lg:gap-5 p-3.5 lg:p-4 text-left",
          "rounded-2xl",
          interactive.transition.default,
          interactive.active.scale,
          "border border-fg/10 bg-fg/5 hover:bg-fg/10",
          disabled && !isOpening && "opacity-60",
        )}
      >
        <AvatarStack group={group} characters={characters} size="lg" />

        <span className="min-w-0 flex-1">
          <span className="flex items-baseline justify-between gap-2">
            <span className={cn(typography.body.size, typography.h3.weight, "truncate text-fg")}>
              {group.name}
            </span>
            {hasSession && !isOpening && (
              <span className={cn(typography.caption.size, "shrink-0 text-fg/40")}>
                {formatTimeAgo(recency)}
              </span>
            )}
          </span>
          <span className="mt-1 flex min-w-0 items-center">{previewNode}</span>
        </span>

        {isOpening ? (
          <Loader2 className="h-4 w-4 shrink-0 animate-spin text-fg/40" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0 text-fg/30" />
        )}
      </button>
    );
  },
);

GroupCard.displayName = "GroupCard";
