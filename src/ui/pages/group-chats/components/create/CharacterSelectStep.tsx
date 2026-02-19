import { motion } from "framer-motion";
import { Users } from "lucide-react";
import type { Character } from "../../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, shadows, cn } from "../../../../design-tokens";
import { CharacterSelectItem } from "./CharacterSelectItem";

interface CharacterSelectStepProps {
  characters: Character[];
  selectedIds: Set<string>;
  onToggleCharacter: (id: string) => void;
  loading: boolean;
  onContinue: () => void;
  canContinue: boolean;
}

export function CharacterSelectStep({
  characters,
  selectedIds,
  onToggleCharacter,
  loading,
  onContinue,
  canContinue,
}: CharacterSelectStepProps) {
  const selectedCount = selectedIds.size;

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className={cn(spacing.section, "flex flex-col flex-1 min-h-0")}
    >
      {/* Title */}
      <div className={spacing.tight}>
        <div className="flex items-center gap-2">
          <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
            <Users className="h-4 w-4 text-accent" />
          </div>
          <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
            Create Group Chat
          </h2>
        </div>
        <p className={cn(typography.body.size, "mt-2 text-fg/50")}>
          Select characters for your group conversation
        </p>
      </div>

      {/* Selection Count */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className={cn(typography.label.size, typography.label.weight, "text-fg/70")}>
            {selectedCount} selected
          </span>
          {selectedCount > 0 && (
            <div
              className={cn(
                "px-2 py-0.5 text-xs font-medium rounded-full",
                selectedCount >= 2
                  ? "bg-accent/20 text-accent/80 border border-accent/30"
                  : "bg-warning/20 text-warning border border-warning/30",
              )}
            >
              {selectedCount >= 2 ? "Ready" : "Min. 2 required"}
            </div>
          )}
        </div>
      </div>

      {/* Character List */}
      <div className="flex-1 overflow-y-auto -mx-4 px-4">
        {loading ? (
          <div className="space-y-2">
            {[0, 1, 2, 3].map((i) => (
              <div
                key={i}
                className={cn(
                  "h-16 animate-pulse",
                  radius.md,
                  "border border-fg/5 bg-fg/5",
                )}
              />
            ))}
          </div>
        ) : characters.length === 0 ? (
          <div
            className={cn(
              "p-8 text-center",
              radius.lg,
              "border border-dashed border-fg/10 bg-fg/2",
            )}
          >
            <Users className="mx-auto h-10 w-10 text-fg/20 mb-3" />
            <p className={cn(typography.body.size, "text-fg/50 mb-1")}>No characters yet</p>
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Create some characters first to start a group chat
            </p>
          </div>
        ) : (
          <div className="space-y-2 pb-4">
            {characters.map((character) => (
              <CharacterSelectItem
                key={character.id}
                character={character}
                selected={selectedIds.has(character.id)}
                onToggle={() => onToggleCharacter(character.id)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Continue Button */}
      <div className="pt-4 mt-auto">
        <motion.button
          disabled={!canContinue}
          onClick={onContinue}
          whileTap={{ scale: canContinue ? 0.97 : 1 }}
          className={cn(
            "w-full py-4 text-base font-semibold",
            radius.md,
            interactive.transition.fast,
            canContinue
              ? cn(
                  "border border-accent/40 bg-accent/20 text-accent",
                  shadows.glow,
                  "active:border-accent/60 active:bg-accent/30",
                )
              : "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30",
          )}
        >
          Continue to Group Setup
        </motion.button>
      </div>
    </motion.div>
  );
}
