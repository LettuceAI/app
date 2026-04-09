import { useEffect, useRef, useState, memo } from "react";
import { ChevronRight, MessageCircle, Plus, Rocket, Users } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

import { useI18n } from "../../../../../core/i18n/context";
import type {
  GroupCharacterPreview,
  GroupSessionPreview,
  Character,
} from "../../../../../core/storage/schemas";

type GroupListItem = Pick<
  GroupSessionPreview | GroupCharacterPreview,
  "id" | "name" | "characterIds"
>;
import { typography, radius, spacing, interactive, cn } from "../../../../design-tokens";
import { useAvatar } from "../../../../hooks/useAvatar";
import { useRocketEasterEgg } from "../../../../hooks/useRocketEasterEgg";
import { AvatarImage } from "../../../../components/AvatarImage";
import { formatTimeAgo } from "../../utils/formatTimeAgo";
import { isRenderableImageUrl } from "../../../../../core/utils/image";

export function GroupSessionList({
  groups,
  allSessions,
  characters,
  expandedGroupId,
  onToggleExpand,
  onSelectSession,
  onNewChat,
  onLongPress,
  onSessionLongPress,
}: {
  groups: GroupListItem[];
  allSessions: GroupSessionPreview[];
  characters: Character[];
  expandedGroupId: string | null;
  onToggleExpand: (group: GroupListItem) => void;
  onSelectSession: (session: GroupSessionPreview) => void;
  onNewChat: (group: GroupListItem) => void;
  onLongPress: (group: GroupListItem) => void;
  onSessionLongPress: (session: GroupSessionPreview) => void;
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
      {groups.slice(0, visibleCount).map((group) => {
        const isExpanded = expandedGroupId === group.id;
        const groupSessions = allSessions
          .filter((s) => s.groupCharacterId === group.id)
          .sort((a, b) => b.updatedAt - a.updatedAt);

        return (
          <div key={group.id}>
            <GroupSessionCard
              session={group}
              characters={characters}
              isExpanded={isExpanded}
              onToggleExpand={onToggleExpand}
              onLongPress={onLongPress}
            />
            <AnimatePresence>
              {isExpanded && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.15 }}
                  className="overflow-hidden"
                >
                  <div className="ml-4 mt-2 space-y-1.5">
                    {groupSessions.map((session) => (
                      <SessionSubItem
                        key={session.id}
                        session={session}
                        onSelect={onSelectSession}
                        onLongPress={onSessionLongPress}
                      />
                    ))}
                    <NewChatRow onClick={() => onNewChat(group)} />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        );
      })}
    </div>
  );
}

function SessionSubItem({
  session,
  onSelect,
  onLongPress,
}: {
  session: GroupSessionPreview;
  onSelect: (session: GroupSessionPreview) => void;
  onLongPress: (session: GroupSessionPreview) => void;
}) {
  const longPressTimer = useRef<number | null>(null);
  const isLongPress = useRef(false);

  const handlePointerDown = () => {
    isLongPress.current = false;
    longPressTimer.current = window.setTimeout(() => {
      isLongPress.current = true;
      onLongPress(session);
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
    onSelect(session);
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    onLongPress(session);
  };

  return (
    <button
      onClick={handleClick}
      onContextMenu={handleContextMenu}
      onPointerDown={handlePointerDown}
      onPointerUp={handlePointerUp}
      onPointerLeave={handlePointerLeave}
      className={cn(
        "flex w-full items-center gap-3 p-3 text-left",
        "rounded-xl",
        interactive.transition.default,
        "border border-fg/10 bg-fg/3 hover:bg-fg/8",
      )}
    >
      <MessageCircle className="h-4 w-4 shrink-0 text-fg/40" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className={cn(typography.caption.size, "font-medium text-fg truncate")}>
            {session.name}
          </span>
          <span className={cn(typography.caption.size, "shrink-0 text-fg/40")}>
            {formatTimeAgo(session.updatedAt)}
          </span>
        </div>
        {session.lastMessage && (
          <p className={cn(typography.caption.size, "text-fg/40 truncate mt-0.5")}>
            {session.lastMessage}
          </p>
        )}
      </div>
      <span className={cn(typography.caption.size, "shrink-0 text-fg/30")}>
        {session.messageCount}
      </span>
    </button>
  );
}

function NewChatRow({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-3 p-3 text-left",
        "rounded-xl",
        interactive.transition.default,
        "border border-dashed border-fg/15 hover:border-accent/40 hover:bg-accent/5",
      )}
    >
      <Plus className="h-4 w-4 text-fg/40" />
      <span className={cn(typography.caption.size, "font-medium text-fg/60")}>
        {t("groupChats.list.newChat")}
      </span>
    </button>
  );
}

export function GroupSessionSkeleton() {
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

const GroupSessionCard = memo(
  ({
    session,
    characters,
    isExpanded,
    onToggleExpand,
    onLongPress,
  }: {
    session: GroupListItem;
    characters: Character[];
    isExpanded: boolean;
    onToggleExpand: (session: GroupListItem) => void;
    onLongPress: (session: GroupListItem) => void;
  }) => {
    const longPressTimer = useRef<number | null>(null);
    const isLongPress = useRef(false);

    const characterNames = session.characterIds
      .map((id) => characters.find((c) => c.id === id)?.name || "Unknown")
      .slice(0, 3)
      .join(", ");

    const extraCount = session.characterIds.length - 3;
    const characterSummary =
      extraCount > 0 ? `${characterNames} +${extraCount} more` : characterNames;

    const handlePointerDown = () => {
      isLongPress.current = false;
      longPressTimer.current = window.setTimeout(() => {
        isLongPress.current = true;
        onLongPress(session);
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
      onToggleExpand(session);
    };

    const handleContextMenu = (e: React.MouseEvent) => {
      e.preventDefault();
      onLongPress(session);
    };

    const avatarCharacters = session.characterIds
      .slice(0, 3)
      .map((id) => characters.find((c) => c.id === id))
      .filter(Boolean) as Character[];

    return (
      <motion.button
        layoutId={`group-session-${session.id}`}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        onPointerDown={handlePointerDown}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerLeave}
        className={cn(
          "group relative flex w-full items-center gap-3.5 lg:gap-6 p-3.5 lg:p-6 text-left",
          "rounded-2xl lg:rounded-3xl",
          interactive.transition.default,
          interactive.active.scale,
          "border border-fg/10 bg-fg/5 hover:bg-fg/10",
        )}
      >
        <div className="flex -space-x-2">
          {avatarCharacters.map((character) => (
            <div
              key={character.id}
              className={cn(
                "flex h-10 w-10 items-center justify-center overflow-hidden rounded-full",
                "border border-fg/10 bg-linear-to-br from-fg/8 to-fg/4",
              )}
            >
              <CharacterMiniAvatar character={character} />
            </div>
          ))}
          {session.characterIds.length > 3 && (
            <div
              className={cn(
                "flex h-10 w-10 items-center justify-center rounded-full",
                "border border-fg/10 bg-fg/10",
                "text-xs font-semibold text-fg/60",
              )}
            >
              +{session.characterIds.length - 3}
            </div>
          )}
        </div>

        <div className="min-w-0 flex-1">
          <h3 className={cn(typography.body.size, typography.h3.weight, "text-fg truncate")}>
            {session.name}
          </h3>
          <p className={cn(typography.caption.size, "text-fg/50 truncate mt-1")}>
            {characterSummary}
          </p>
        </div>

        <motion.div animate={{ rotate: isExpanded ? 90 : 0 }} transition={{ duration: 0.15 }}>
          <ChevronRight className="h-4 w-4 text-fg/30" />
        </motion.div>
      </motion.button>
    );
  },
);

GroupSessionCard.displayName = "GroupSessionCard";
