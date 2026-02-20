import { useEffect, useState, memo, useRef } from "react";
import { Rocket, Users } from "lucide-react";
import { motion } from "framer-motion";

import type { GroupSessionPreview, Character } from "../../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, cn } from "../../../../design-tokens";
import { useAvatar } from "../../../../hooks/useAvatar";
import { useRocketEasterEgg } from "../../../../hooks/useRocketEasterEgg";
import { AvatarImage } from "../../../../components/AvatarImage";

export function GroupSessionList({
  sessions,
  characters,
  onSelect,
  onLongPress,
}: {
  sessions: GroupSessionPreview[];
  characters: Character[];
  onSelect: (session: GroupSessionPreview) => void;
  onLongPress: (session: GroupSessionPreview) => void;
}) {
  const [visibleCount, setVisibleCount] = useState(10);

  useEffect(() => {
    if (visibleCount < sessions.length) {
      const timer = setTimeout(() => {
        setVisibleCount((prev) => Math.min(prev + 10, sessions.length));
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [visibleCount, sessions.length]);

  useEffect(() => {
    setVisibleCount(10);
  }, [sessions]);

  return (
    <div className="space-y-2 lg:space-y-3 pb-24">
      {sessions.slice(0, visibleCount).map((session) => (
        <GroupSessionCard
          key={session.id}
          session={session}
          characters={characters}
          onSelect={onSelect}
          onLongPress={onLongPress}
        />
      ))}
    </div>
  );
}

export function GroupSessionSkeleton() {
  return (
    <div className={spacing.item}>
      {[0, 1, 2].map((index) => (
        <div
          key={index}
          className={cn(
            "h-20 animate-pulse p-4",
            "rounded-2xl",
            "border border-fg/5 bg-fg/5",
          )}
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
          No group chats yet
        </h3>
        <p className={cn(typography.body.size, typography.body.lineHeight, "text-fg/50")}>
          Create your first group chat from the + button below to have conversations with multiple
          characters at once
        </p>
      </div>
    </div>
  );
}

function isImageLike(s?: string) {
  if (!s) return false;
  const lower = s.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
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
    onSelect,
    onLongPress,
  }: {
    session: GroupSessionPreview;
    characters: Character[];
    onSelect: (session: GroupSessionPreview) => void;
    onLongPress: (session: GroupSessionPreview) => void;
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
      onSelect(session);
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
                "border border-fg/10 bg-linear-to-br from-white/10 to-white/5",
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
      </motion.button>
    );
  },
);

GroupSessionCard.displayName = "GroupSessionCard";
