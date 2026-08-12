import { useMemo, useState } from "react";
import { motion } from "framer-motion";
import { Search, Upload, Users } from "lucide-react";
import { useI18n } from "../../../../../core/i18n/context";
import type { Character } from "../../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, cn } from "../../../../design-tokens";
import { CharacterSelectItem } from "./CharacterSelectItem";

interface CharacterSelectStepProps {
  characters: Character[];
  selectedIds: Set<string>;
  onToggleCharacter: (id: string) => void;
  loading: boolean;
  onImport: () => void;
}

export function CharacterSelectStep({
  characters,
  selectedIds,
  onToggleCharacter,
  loading,
  onImport,
}: CharacterSelectStepProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");

  const filteredCharacters = useMemo(() => {
    const trimmed = query.trim().toLowerCase();
    if (!trimmed) return characters;
    return characters.filter((c) => {
      const description = `${c.description ?? ""} ${c.definition ?? ""}`.toLowerCase();
      return c.name.toLowerCase().includes(trimmed) || description.includes(trimmed);
    });
  }, [characters, query]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className={spacing.group}
    >
      <div>
        <h1 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
          {t("groupChats.create.characterSelect.title")}
        </h1>
        <p className={cn(typography.body.size, "mt-1 text-fg/50")}>
          {t("groupChats.create.characterSelect.subtitle")}
        </p>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("groupChats.create.characterSelect.searchPlaceholder")}
            className={cn(
              "w-full border border-fg/10 bg-surface-el/85 py-2.5 pl-9 pr-3",
              radius.lg,
              typography.body.size,
              "text-fg placeholder:text-fg/40",
              interactive.transition.default,
              "focus:border-fg/30 focus:outline-none",
            )}
          />
        </div>
        <button
          type="button"
          onClick={onImport}
          className={cn(
            "flex shrink-0 items-center gap-1.5 border border-fg/10 bg-surface-el/85 px-3 py-2.5",
            radius.lg,
            "text-xs font-medium text-fg/70",
            interactive.transition.fast,
            "hover:border-fg/20 hover:text-fg active:scale-[0.97]",
          )}
        >
          <Upload className="h-3.5 w-3.5" />
          {t("groupChats.create.importChatpkg")}
        </button>
      </div>

      {loading ? (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={cn("h-16 animate-pulse", radius.lg, "border border-fg/5 bg-fg/5")}
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
          <Users className="mx-auto mb-3 h-10 w-10 text-fg/20" />
          <p className={cn(typography.body.size, "mb-1 text-fg/50")}>
            {t("groupChats.create.characterSelect.noCharactersYet")}
          </p>
          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("groupChats.create.characterSelect.noCharactersDesc")}
          </p>
        </div>
      ) : filteredCharacters.length === 0 ? (
        <div
          className={cn(
            "p-8 text-center",
            radius.lg,
            "border border-dashed border-fg/10 bg-fg/2",
          )}
        >
          <p className={cn(typography.body.size, "text-fg/50")}>
            {t("groupChats.create.characterSelect.noResults")}
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-2 pb-2 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
          {filteredCharacters.map((character) => (
            <CharacterSelectItem
              key={character.id}
              character={character}
              selected={selectedIds.has(character.id)}
              onToggle={() => onToggleCharacter(character.id)}
            />
          ))}
        </div>
      )}
    </motion.div>
  );
}
