import type { Character } from "../../../../../core/storage/schemas";
import { AvatarImage } from "../../../../components/AvatarImage";
import { useAvatar } from "../../../../hooks/useAvatar";
import { cn } from "../../../../design-tokens";

export function CharacterAvatar({
  character,
  size = "md",
}: {
  character: Character;
  size?: "sm" | "md" | "lg";
}) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  const sizeClasses = {
    sm: "h-8 w-8 text-xs",
    md: "h-10 w-10 text-sm",
    lg: "h-12 w-12 text-base",
  };

  return (
    <div
      className={cn(
        sizeClasses[size],
        "rounded-full overflow-hidden",
        "bg-linear-to-br from-white/10 to-white/5",
        "border border-fg/10",
        "flex items-center justify-center",
      )}
    >
      {avatarUrl ? (
        <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
      ) : (
        <span className="font-bold text-fg/60">{character.name.slice(0, 2).toUpperCase()}</span>
      )}
    </div>
  );
}
