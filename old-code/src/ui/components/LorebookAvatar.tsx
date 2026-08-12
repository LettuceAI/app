import { useImageData } from "../hooks/useImageData";
import { cn } from "../design-tokens";
import { useI18n } from "../../core/i18n/context";

type LorebookAvatarProps = {
  avatarPath?: string | null;
  name?: string;
  className?: string;
  imageClassName?: string;
  fallbackClassName?: string;
  /** @deprecated retained for backwards compatibility; ignored by the letter fallback. */
  iconClassName?: string;
};

function getInitial(name?: string): string {
  if (!name) return "·";
  const trimmed = name.trim();
  if (!trimmed) return "·";
  const first = Array.from(trimmed)[0];
  return first ? first.toUpperCase() : "·";
}

export function LorebookAvatar({
  avatarPath,
  name,
  className,
  imageClassName,
  fallbackClassName,
}: LorebookAvatarProps) {
  const { t } = useI18n();
  const avatarUrl = useImageData(avatarPath);

  if (avatarUrl) {
    return (
      <img
        src={avatarUrl}
        alt={name ? t("components.lorebookAvatar.altNamed", { name }) : t("components.lorebookAvatar.alt")}
        loading="lazy"
        decoding="async"
        className={cn("h-full w-full object-cover", className, imageClassName)}
      />
    );
  }

  return (
    <div
      className={cn(
        "flex h-full w-full items-center justify-center bg-fg/8 font-semibold text-fg/65",
        className,
        fallbackClassName,
      )}
    >
      <span className="text-[0.65em] uppercase leading-none tracking-tight">
        {getInitial(name)}
      </span>
    </div>
  );
}
