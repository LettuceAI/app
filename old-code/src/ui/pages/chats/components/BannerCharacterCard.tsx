import { memo, useRef } from "react";
import type { Character } from "../../../../core/storage/schemas";
import { cn, interactive } from "../../../design-tokens";
import { useAvatarGradient } from "../../../hooks/useAvatarGradient";
import { useI18n } from "../../../../core/i18n/context";
import { CharacterAvatar } from "./CharacterAvatar";

export interface BannerCharacterCardProps {
  character: Character;
  onSelect: (character: Character) => void;
  onLongPress: (character: Character) => void;
}

export const BannerCharacterCard = memo(
  ({ character, onSelect, onLongPress }: BannerCharacterCardProps) => {
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
          "group relative flex w-full text-left overflow-hidden",
          "h-24 lg:h-36",
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
        {/* Cover Avatar — fills left, fades on its right edge.
            max-w prevents the slot from stretching into a thin horizontal strip
            on very wide viewports (list view at desktop width). */}
        <div
          aria-hidden
          className="absolute inset-y-0 left-0 w-3/5 lg:w-1/2 max-w-[420px] overflow-hidden"
          style={{
            WebkitMaskImage:
              "linear-gradient(to right, black 0%, black 55%, transparent 100%)",
            maskImage:
              "linear-gradient(to right, black 0%, black 55%, transparent 100%)",
          }}
        >
          <div className="h-full w-full origin-center scale-[1.14] transition-transform duration-500 ease-out group-hover:-translate-x-3 group-hover:scale-[1.14]">
            <CharacterAvatar character={character} variant="banner" />
          </div>
        </div>

        {/* Darkening scrim — gives the text panel contrast over the gradient */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 transition-opacity duration-300 group-hover:opacity-90"
          style={{
            background:
              "linear-gradient(to right, rgba(0,0,0,0) 28%, rgba(0,0,0,0.45) 60%, rgba(0,0,0,0.55) 100%)",
          }}
        />

        {/* Subtle highlight glow on hover */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-300 group-hover:opacity-100"
          style={{
            background:
              "radial-gradient(120% 80% at 30% 50%, rgba(255,255,255,0.06) 0%, transparent 60%)",
          }}
        />

        {/* Content panel — sits on the right side over the gradient */}
        <div className="relative z-10 flex w-full items-center gap-3 pl-[42%] lg:pl-[36%] pr-4 lg:pr-6 py-3.5 lg:py-6">
          <div className="flex min-w-0 flex-1 flex-col gap-0.5 lg:gap-1.5">
            <h3
              className={cn(
                "truncate font-semibold text-[15px] lg:text-xl leading-tight drop-shadow-sm",
                hasGradient ? "" : "text-white",
              )}
              style={hasGradient ? { color: textColor } : {}}
            >
              {character.name}
            </h3>
            <p
              className={cn(
                "line-clamp-2 lg:line-clamp-3 text-[12px] lg:text-sm leading-snug lg:leading-relaxed",
                hasGradient ? "" : "text-white/50",
              )}
              style={hasGradient ? { color: textSecondary } : {}}
            >
              {descriptionPreview}
            </p>
          </div>

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
              "shrink-0 transition-all duration-300 group-hover:translate-x-0.5",
              hasGradient ? "" : "text-white/30 group-hover:text-white/60",
            )}
            style={hasGradient ? { color: textSecondary } : {}}
          >
            <path d="m9 18 6-6-6-6" />
          </svg>
        </div>
      </button>
    );
  },
);

BannerCharacterCard.displayName = "BannerCharacterCard";
