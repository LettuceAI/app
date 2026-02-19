import { useState, useEffect, useMemo, memo } from "react";
import { Search, User, Users, X } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { cn, typography, radius, interactive } from "../../../design-tokens";
import { BottomMenu } from "../../../components";
import { listCharacters, listPersonas } from "../../../../core/storage/repo";
import { useAvatar } from "../../../hooks/useAvatar";
import { AvatarImage } from "../../../components/AvatarImage";
import type { AvatarCrop, Character, Persona } from "../../../../core/storage/schemas";

export interface Reference {
  type: "character" | "persona";
  id: string;
  name: string;
  description?: string;
  avatarPath?: string;
  avatarCrop?: AvatarCrop | null;
}

interface ReferenceSelectorProps {
  isOpen: boolean;
  onClose: () => void;
  type: "character" | "persona";
  onSelect: (ref: Reference) => void;
  existingRefs?: Reference[];
}

function isImageLike(s?: string) {
  if (!s) return false;
  const lower = s.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
}

// Avatar component that uses the useAvatar hook
export const ReferenceAvatar = memo(
  ({
    type,
    id,
    avatarPath,
    avatarCrop,
    name,
    size = "md",
  }: {
    type: "character" | "persona";
    id: string;
    avatarPath?: string;
    avatarCrop?: AvatarCrop | null;
    name: string;
    size?: "sm" | "md";
  }) => {
    const avatarUrl = useAvatar(type, id, avatarPath, "round");
    const sizeClasses = size === "sm" ? "h-5 w-5" : "h-10 w-10";
    const iconSize = size === "sm" ? "h-3 w-3" : "h-5 w-5";

    if (avatarUrl && isImageLike(avatarUrl)) {
      return (
        <div className={cn(sizeClasses, "rounded-full overflow-hidden shrink-0")}>
          <AvatarImage src={avatarUrl} alt={name} crop={avatarCrop} applyCrop />
        </div>
      );
    }

    // Fallback icon
    return (
      <div
        className={cn(
          sizeClasses,
          "flex shrink-0 items-center justify-center rounded-full",
          type === "character" ? "bg-secondary/20" : "bg-warning/20",
        )}
      >
        {type === "character" ? (
          <User className={cn(iconSize, "text-secondary")} />
        ) : (
          <Users className={cn(iconSize, "text-warning")} />
        )}
      </div>
    );
  },
);

ReferenceAvatar.displayName = "ReferenceAvatar";

export function ReferenceSelector({
  isOpen,
  onClose,
  type,
  onSelect,
  existingRefs = [],
}: ReferenceSelectorProps) {
  const [search, setSearch] = useState("");
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isOpen) return;

    const load = async () => {
      setLoading(true);
      try {
        if (type === "character") {
          const chars = await listCharacters();
          setCharacters(chars);
        } else {
          const pers = await listPersonas();
          setPersonas(pers);
        }
      } catch (err) {
        console.error("Failed to load references:", err);
      } finally {
        setLoading(false);
      }
    };

    load();
  }, [isOpen, type]);

  const filteredItems = useMemo(() => {
    const searchLower = search.toLowerCase();
    const existingIds = new Set(existingRefs.map((r) => r.id));

    if (type === "character") {
      return characters.filter((c) => {
        const desc = `${c.description ?? ""} ${c.definition ?? ""}`.toLowerCase();
        return (
          !existingIds.has(c.id) &&
          (c.name.toLowerCase().includes(searchLower) || desc.includes(searchLower))
        );
      });
    } else {
      return personas.filter(
        (p) =>
          !existingIds.has(p.id) &&
          (p.title.toLowerCase().includes(searchLower) ||
            p.description.toLowerCase().includes(searchLower)),
      );
    }
  }, [type, characters, personas, search, existingRefs]);

  const handleSelect = (item: Character | Persona) => {
    if (type === "character") {
      const char = item as Character;
      onSelect({
        type: "character",
        id: char.id,
        name: char.name,
        description: char.definition || char.description,
        avatarPath: char.avatarPath,
        avatarCrop: char.avatarCrop ?? null,
      });
    } else {
      const persona = item as Persona;
      onSelect({
        type: "persona",
        id: persona.id,
        name: persona.title,
        description: persona.description,
        avatarPath: persona.avatarPath,
        avatarCrop: persona.avatarCrop ?? null,
      });
    }
    onClose();
  };

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={type === "character" ? "Select Character" : "Select Persona"}
    >
      <div className="space-y-4">
        {/* Search Input */}
        <div className="relative">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={`Search ${type}s...`}
            className={cn(
              "w-full py-2.5 pl-10 pr-4",
              radius.lg,
              "bg-fg/5 border border-fg/15",
              "text-fg placeholder-fg/40 text-sm",
              "focus:outline-none focus:border-fg/30",
            )}
          />
          {search && (
            <button
              onClick={() => setSearch("")}
              className="absolute right-3 top-1/2 -translate-y-1/2"
            >
              <X className="h-4 w-4 text-fg/40" />
            </button>
          )}
        </div>

        {/* List */}
        <div className="max-h-[50vh] overflow-y-auto -mx-4 px-4">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <span className="text-fg/40 text-sm">Loading...</span>
            </div>
          ) : filteredItems.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-center">
              <div className="mb-2 h-10 w-10 rounded-full bg-fg/5 flex items-center justify-center">
                {type === "character" ? (
                  <User className="h-5 w-5 text-fg/30" />
                ) : (
                  <Users className="h-5 w-5 text-fg/30" />
                )}
              </div>
              <span className="text-fg/40 text-sm">
                {search ? `No ${type}s match your search` : `No ${type}s available`}
              </span>
            </div>
          ) : (
            <AnimatePresence mode="popLayout">
              <div className="space-y-2">
                {filteredItems.map((item) => {
                  const name =
                    type === "character" ? (item as Character).name : (item as Persona).title;
                  const desc =
                    type === "character"
                      ? (item as Character).description || (item as Character).definition
                      : (item as Persona).description;
                  const avatarPath =
                    type === "character"
                      ? (item as Character).avatarPath
                      : (item as Persona).avatarPath;
                  const avatarCrop =
                    type === "character"
                      ? (item as Character).avatarCrop
                      : (item as Persona).avatarCrop;

                  return (
                    <motion.button
                      key={item.id}
                      initial={{ opacity: 0, y: 5 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      onClick={() => handleSelect(item)}
                      className={cn(
                        "w-full text-left p-3",
                        radius.lg,
                        "bg-fg/5 border border-fg/10",
                        interactive.transition.fast,
                        "hover:bg-fg/10 hover:border-fg/20",
                      )}
                    >
                      <div className="flex items-start gap-3">
                        <ReferenceAvatar
                          type={type}
                          id={item.id}
                          avatarPath={avatarPath}
                          avatarCrop={avatarCrop}
                          name={name}
                          size="md"
                        />
                        <div className="flex-1 min-w-0">
                          <div
                            className={cn(typography.body.size, "font-medium text-fg truncate")}
                          >
                            {name}
                          </div>
                          {desc && (
                            <div className="text-sm text-fg/50 line-clamp-2 mt-0.5">{desc}</div>
                          )}
                        </div>
                      </div>
                    </motion.button>
                  );
                })}
              </div>
            </AnimatePresence>
          )}
        </div>
      </div>
    </BottomMenu>
  );
}
