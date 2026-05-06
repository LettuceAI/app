import { useMemo, useState } from "react";
import { Check, Search, User, UserX } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";

import { BottomMenu, MenuSection } from "../../../../components/BottomMenu";
import { cn, interactive, spacing, typography } from "../../../../design-tokens";
import { useAvatar } from "../../../../hooks/useAvatar";
import { AvatarImage } from "../../../../components/AvatarImage";
import type { Character } from "../../../../../core/storage/schemas";

interface CharacterAvatarProps {
  character: Character | null;
  size?: "sm" | "md" | "lg";
  isSelected?: boolean;
}

function CharacterEntityAvatar({
  character,
  size = "md",
  isSelected = false,
}: CharacterAvatarProps) {
  const avatarDataUrl = useAvatar(
    "character",
    character?.id ?? "",
    character?.avatarPath,
    "round",
  );

  const sizeClasses = {
    sm: "h-8 w-8",
    md: "h-10 w-10",
    lg: "h-12 w-12",
  };

  const iconSizes = {
    sm: "h-4 w-4",
    md: "h-5 w-5",
    lg: "h-6 w-6",
  };

  if (!character) {
    return (
      <div
        className={cn(
          sizeClasses[size],
          "relative flex shrink-0 items-center justify-center overflow-hidden rounded-full border",
          isSelected ? "border-accent/40 bg-accent/15" : "border-fg/15 bg-fg/5",
        )}
      >
        <UserX className={cn(iconSizes[size], isSelected ? "text-accent/80" : "text-fg/50")} />
      </div>
    );
  }

  return (
    <div
      className={cn(
        sizeClasses[size],
        "relative flex shrink-0 items-center justify-center overflow-hidden rounded-full border",
        isSelected ? "border-accent/40 bg-accent/15" : "border-fg/15 bg-fg/5",
      )}
    >
      {avatarDataUrl ? (
        <AvatarImage
          src={avatarDataUrl}
          alt={character.name}
          crop={character.avatarCrop}
          applyCrop
        />
      ) : (
        <User className={cn(iconSizes[size], isSelected ? "text-accent/80" : "text-fg/60")} />
      )}
    </div>
  );
}

interface CharacterOptionItemProps {
  character: Character | null;
  isSelected: boolean;
  onClick: () => void;
}

function CharacterOptionItem({ character, isSelected, onClick }: CharacterOptionItemProps) {
  const title = character?.name ?? "No Character";
  const description =
    character?.description ?? character?.definition ?? "Continue without a character";

  return (
    <motion.button
      layout="position"
      initial={false}
      transition={{ duration: 0.12, ease: "easeOut" }}
      onClick={onClick}
      className={cn(
        "group relative flex w-full items-center gap-3",
        "rounded-xl px-3 py-3 text-left",
        interactive.transition.default,
        "active:scale-[0.98]",
        isSelected
          ? "border border-accent/40 bg-accent/10 ring-1 ring-accent/20"
          : "border border-fg/8 bg-fg/3 hover:border-fg/15 hover:bg-fg/6",
      )}
      aria-pressed={isSelected}
    >
      <CharacterEntityAvatar character={character} size="md" isSelected={isSelected} />

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 overflow-hidden">
          <span
            className={cn(
              typography.body.size,
              "truncate font-medium",
              isSelected ? "text-accent" : "text-fg",
            )}
          >
            {title}
          </span>
        </div>
        <p
          className={cn(
            typography.caption.size,
            "mt-0.5 truncate",
            isSelected ? "text-accent/70" : "text-fg/50",
          )}
        >
          {description}
        </p>
      </div>

      <div
        className={cn(
          "flex h-6 w-6 shrink-0 items-center justify-center rounded-full border transition-all",
          isSelected
            ? "border-accent/60 bg-accent/30 text-accent/80"
            : "border-fg/15 bg-fg/5 text-transparent group-hover:border-fg/25",
        )}
      >
        <Check className={cn("h-3.5 w-3.5", isSelected ? "opacity-100" : "opacity-0")} />
      </div>
    </motion.button>
  );
}

interface CharacterSelectorSingleProps {
  isOpen: boolean;
  onClose: () => void;
  characters: Character[];
  selectedCharacterId: string | null | undefined;
  onSelect: (characterId: string | null) => void;
  showSearch?: boolean;
  title?: string;
}

export function CharacterSelectorSingle({
  isOpen,
  onClose,
  characters,
  selectedCharacterId,
  onSelect,
  showSearch = true,
  title = "Select Character",
}: CharacterSelectorSingleProps) {
  const [searchQuery, setSearchQuery] = useState("");

  const filteredCharacters = useMemo(() => {
    if (!searchQuery.trim()) return characters;
    const q = searchQuery.toLowerCase();
    return characters.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        (c.description ?? "").toLowerCase().includes(q) ||
        (c.definition ?? "").toLowerCase().includes(q),
    );
  }, [characters, searchQuery]);

  const sortedCharacters = useMemo(() => {
    return [...filteredCharacters].sort((a, b) => a.name.localeCompare(b.name));
  }, [filteredCharacters]);

  const handleSelect = (characterId: string | null) => {
    onSelect(characterId);
    onClose();
    setSearchQuery("");
  };

  const handleClose = () => {
    onClose();
    setSearchQuery("");
  };

  const showSearchBar = showSearch && characters.length > 3;

  return (
    <BottomMenu isOpen={isOpen} onClose={handleClose} title={title}>
      <div className={spacing.group}>
        {characters.length === 0 ? (
          <div className={cn("rounded-xl", "border border-warning/20 bg-warning/10 px-5 py-4")}>
            <div className="flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-warning/30 bg-warning/10">
                <User className="h-4 w-4 text-warning" />
              </div>
              <div>
                <p className={cn(typography.body.size, "font-medium text-warning")}>
                  No characters available
                </p>
                <p className={cn(typography.caption.size, "mt-1 text-warning/60")}>
                  Create a character first to use it as a generation source.
                </p>
              </div>
            </div>
          </div>
        ) : (
          <MenuSection>
            {showSearchBar && (
              <div className="relative mb-3">
                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search characters..."
                  className={cn(
                    "w-full rounded-xl border border-fg/10 bg-surface-el/30",
                    "px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40",
                    "focus:border-fg/20 focus:outline-none",
                    interactive.transition.default,
                  )}
                />
              </div>
            )}

            <div className="-mx-1 max-h-[50vh] space-y-2 overflow-y-auto px-1">
              <AnimatePresence initial={false}>
                <CharacterOptionItem
                  key="no-character"
                  character={null}
                  isSelected={!selectedCharacterId}
                  onClick={() => handleSelect(null)}
                />

                {sortedCharacters.map((character) => (
                  <CharacterOptionItem
                    key={character.id}
                    character={character}
                    isSelected={character.id === selectedCharacterId}
                    onClick={() => handleSelect(character.id)}
                  />
                ))}
              </AnimatePresence>

              {searchQuery && filteredCharacters.length === 0 && (
                <div className="py-8 text-center">
                  <User className="mx-auto h-8 w-8 text-fg/20" />
                  <p className={cn(typography.body.size, "mt-3 text-fg/50")}>
                    No characters found
                  </p>
                  <p className={cn(typography.caption.size, "mt-1 text-fg/30")}>
                    Try a different search term
                  </p>
                </div>
              )}
            </div>
          </MenuSection>
        )}
      </div>
    </BottomMenu>
  );
}
