import { Check } from "lucide-react";

import type { Character } from "../../../../../core/storage/schemas";
import { AvatarImage } from "../../../../components/AvatarImage";
import { typography, radius, interactive, cn } from "../../../../design-tokens";
import { useAvatar } from "../../../../hooks/useAvatar";

type CharacterSelectItemProps = {
  character: Character;
  selected: boolean;
  onToggle: () => void;
};

export function CharacterSelectItem({ character, selected, onToggle }: CharacterSelectItemProps) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");
  const description = character.description || character.definition;

  return (
    <button
      onClick={onToggle}
      aria-pressed={selected}
      className={cn(
        "flex w-full items-center gap-3 p-2.5 text-left",
        radius.lg,
        "border",
        interactive.transition.fast,
        selected
          ? "border-accent/40 bg-accent/10"
          : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
      )}
    >
      <div
        className={cn(
          "h-11 w-11 shrink-0 overflow-hidden rounded-full",
          "bg-linear-to-br from-fg/8 to-fg/4",
          selected ? "ring-2 ring-accent/50" : "ring-1 ring-fg/10",
        )}
      >
        {avatarUrl ? (
          <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-sm font-bold text-fg/60">
            {character.name.slice(0, 2).toUpperCase()}
          </div>
        )}
      </div>

      <div className="min-w-0 flex-1">
        <p
          className={cn(
            "truncate font-medium",
            typography.body.size,
            selected ? "text-accent" : "text-fg",
          )}
        >
          {character.name}
        </p>
        {description && (
          <p className={cn("truncate", typography.bodySmall.size, "text-fg/50")}>{description}</p>
        )}
      </div>

      <div
        className={cn(
          "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border",
          interactive.transition.fast,
          selected ? "border-accent bg-accent text-surface" : "border-fg/25 text-transparent",
        )}
      >
        <Check size={12} strokeWidth={3} />
      </div>
    </button>
  );
}
