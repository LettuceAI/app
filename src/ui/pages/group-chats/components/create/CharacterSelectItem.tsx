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
      className={cn(
        "w-full flex items-center gap-3 p-3 text-left",
        radius.md,
        "border transition",
        selected
          ? "border-accent/40 bg-accent/10"
          : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/10",
        interactive.transition.fast,
      )}
    >
      <div
        className={cn(
          "relative h-12 w-12 shrink-0 overflow-hidden rounded-full",
          "bg-linear-to-br from-white/10 to-white/5",
          selected ? "ring-2 ring-accent/50" : "ring-1 ring-white/10",
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

      <div className="flex-1 min-w-0">
        <h3
          className={cn(
            "truncate font-medium",
            typography.body.size,
            selected ? "text-accent" : "text-fg",
          )}
        >
          {character.name}
        </h3>
        {description && (
          <p className={cn("truncate text-sm", selected ? "text-accent/60" : "text-fg/50")}>
            {description}
          </p>
        )}
      </div>

      <div
        className={cn(
          "h-6 w-6 shrink-0 rounded-full flex items-center justify-center",
          "border transition",
          selected
            ? "border-accent bg-accent text-surface"
            : "border-fg/30 bg-transparent",
        )}
      >
        {selected && <Check size={14} strokeWidth={3} />}
      </div>
    </button>
  );
}
