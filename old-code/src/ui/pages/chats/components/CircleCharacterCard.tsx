import { memo, useRef } from "react";
import type { Character } from "../../../../core/storage/schemas";
import { cn, interactive } from "../../../design-tokens";
import { useAvatarGradient } from "../../../hooks/useAvatarGradient";
import { useI18n } from "../../../../core/i18n/context";
import { CharacterAvatar } from "./CharacterAvatar";

export interface CircleCharacterCardProps {
  character: Character;
  onSelect: (character: Character) => void;
  onLongPress: (character: Character) => void;
}

export const CircleCharacterCard = memo(
  ({ character, onSelect, onLongPress }: CircleCharacterCardProps) => {
    const { t } = useI18n();
    const descriptionPreview =
      (character.description || character.definition || "").trim() ||
      t("common.labels.noDescriptionYet");
    const { gradientCss, hasGradient, textColor, textSecondary } = useAvatarGradient(
      "character",
      character.id,
      character.avatarPath,
      character.disableAvatarGradient,
      character.customGradientEnabled && character.customGradientColors?.length
        ? {
            colors: character.customGradientColors,
            textColor: character.customTextColor,
            textSecondary: character.customTextSecondary,
          }
        : undefined,
      character.avatarGradientSource ?? "base",
    );

    const longPressTimer = useRef<number | null>(null);
    const isLongPress = useRef(false);

    const handlePointerDown = () => {
      isLongPress.current = false;
      longPressTimer.current = window.setTimeout(() => {
        isLongPress.current = true;
        onLongPress(character);
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
      onSelect(character);
    };

    const handleContextMenu = (e: React.MouseEvent) => {
      e.preventDefault();
      onLongPress(character);
    };

    return (
      <button
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        onPointerDown={handlePointerDown}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerLeave}
        className={cn(
          "group relative flex w-full items-center gap-3.5 lg:gap-6 p-3.5 lg:p-6 text-left overflow-hidden",
          "rounded-2xl lg:rounded-3xl",
          interactive.transition.default,
          interactive.active.scale,
          hasGradient ? "" : "border border-white/10 bg-[#1a1b23] hover:bg-[#22232d]",
        )}
        style={
          hasGradient
            ? {
                background: gradientCss,
                boxShadow: "inset 0 0 0 1px rgba(255, 255, 255, 0.12)",
              }
            : {}
        }
      >
        {/* Circular Avatar */}
        <div
          className={cn(
            "relative h-14 w-14 lg:h-24 lg:w-24 shrink-0 overflow-hidden rounded-full",
            hasGradient ? "ring-2 ring-white/25" : "ring-1 ring-white/15",
            "shadow-lg",
          )}
        >
          <CharacterAvatar character={character} />
        </div>

        {/* Content */}
        <div className="flex min-w-0 flex-1 flex-col gap-0.5 lg:gap-1.5 py-1">
          <h3
            className={cn(
              "truncate font-semibold text-[15px] lg:text-xl leading-tight",
              hasGradient ? "" : "text-white",
            )}
            style={hasGradient ? { color: textColor } : {}}
          >
            {character.name}
          </h3>
          <p
            className={cn(
              "line-clamp-1 lg:line-clamp-2 text-[13px] lg:text-base leading-tight lg:leading-relaxed",
              hasGradient ? "" : "text-white/50",
            )}
            style={hasGradient ? { color: textSecondary } : {}}
          >
            {descriptionPreview}
          </p>
        </div>

        {/* chevron */}
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={cn(
            "shrink-0 transition-all",
            hasGradient ? "" : "text-white/30 group-hover:text-white/60",
          )}
          style={hasGradient ? { color: textSecondary } : {}}
        >
          <path d="m9 18 6-6-6-6" />
        </svg>
      </button>
    );
  },
);

CircleCharacterCard.displayName = "CircleCharacterCard";
