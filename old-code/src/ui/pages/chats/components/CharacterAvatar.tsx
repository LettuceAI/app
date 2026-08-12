import { memo } from "react";
import type { Character } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { useAvatar, type AvatarVariant } from "../../../hooks/useAvatar";
import { cn } from "../../../design-tokens";
import { isRenderableImageUrl } from "../../../../core/utils/image";
import { useI18n } from "../../../../core/i18n/context";

function isImageLike(s?: string) {
  return isRenderableImageUrl(s);
}

interface CharacterAvatarProps {
  character: Character;
  className?: string;
  /** "round" (default) — small round avatar, "base" — full base image, "banner" — wide banner with base fallback */
  variant?: AvatarVariant;
}

export const CharacterAvatar = memo(
  ({ character, className, variant = "round" }: CharacterAvatarProps) => {
    const { t } = useI18n();
    const avatarUrl = useAvatar("character", character.id, character.avatarPath, variant);

    if (avatarUrl && isImageLike(avatarUrl)) {
      // For "round" we honor the per-character crop (the cropper produces a square crop).
      // For "base"/"banner" the displayed image is already cropped/processed, so we let it cover the container.
      const applyCrop = variant === "round";
      const crop = variant === "banner" ? character.bannerCrop : character.avatarCrop;
      return (
        <AvatarImage
          src={avatarUrl}
          alt={t("chats.characterAvatarAlt", { name: character.name })}
          crop={crop}
          applyCrop={applyCrop}
          className={className}
        />
      );
    }

    const initials = character.name.slice(0, 2).toUpperCase();
    const initialsSize =
      variant === "round" ? "text-lg" : variant === "banner" ? "text-2xl" : "text-3xl";
    return (
      <div
        className={cn(
          "flex h-full w-full items-center justify-center",
          "bg-linear-to-br from-white/20 to-white/5",
          className,
        )}
      >
        <span className={cn(initialsSize, "font-bold text-white/80")}>{initials}</span>
      </div>
    );
  },
);

CharacterAvatar.displayName = "CharacterAvatar";
