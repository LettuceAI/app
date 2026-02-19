import { useState, useMemo, useRef } from "react";
import type { TouchEvent } from "react";
import { User, Check, Search, UserX, Star } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

import { BottomMenu, MenuSection } from "../../../../components/BottomMenu";
import { cn, typography, interactive, spacing } from "../../../../design-tokens";
import { useAvatar } from "../../../../hooks/useAvatar";
import { AvatarImage } from "../../../../components/AvatarImage";
import type { Persona } from "../../../../../core/storage/schemas";

interface PersonaAvatarProps {
  persona: Persona | null;
  size?: "sm" | "md" | "lg";
  isSelected?: boolean;
}

function PersonaAvatar({ persona, size = "md", isSelected = false }: PersonaAvatarProps) {
  const avatarDataUrl = useAvatar("persona", persona?.id ?? "", persona?.avatarPath, "round");

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

  if (!persona) {
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
        isSelected
          ? "border-accent/40 bg-accent/15"
          : persona.isDefault
            ? "border-info/30 bg-info/10"
            : "border-fg/15 bg-fg/5",
      )}
    >
      {avatarDataUrl ? (
        <AvatarImage src={avatarDataUrl} alt={persona.title} crop={persona.avatarCrop} applyCrop />
      ) : (
        <User
          className={cn(
            iconSizes[size],
            isSelected ? "text-accent/80" : persona.isDefault ? "text-info" : "text-fg/60",
          )}
        />
      )}
    </div>
  );
}

interface PersonaOptionItemProps {
  persona: Persona | null;
  isSelected: boolean;
  onClick: () => void;
  onLongPress?: () => void;
}

function PersonaOptionItem({ persona, isSelected, onClick, onLongPress }: PersonaOptionItemProps) {
  const [longPressTimer, setLongPressTimer] = useState<number | null>(null);
  const [isLongPressTriggered, setIsLongPressTriggered] = useState(false);
  const touchStartRef = useRef<{ x: number; y: number } | null>(null);
  const didMoveRef = useRef(false);
  const suppressClickRef = useRef(false);

  const handleTouchStart = (event: TouchEvent<HTMLButtonElement>) => {
    setIsLongPressTriggered(false);
    const touch = event.touches[0];
    touchStartRef.current = touch ? { x: touch.clientX, y: touch.clientY } : null;
    didMoveRef.current = false;
    suppressClickRef.current = false;
    if (onLongPress) {
      const timer = window.setTimeout(() => {
        setIsLongPressTriggered(true);
        onLongPress();
      }, 500);
      setLongPressTimer(timer);
    }
  };

  const handleTouchMove = (event: TouchEvent<HTMLButtonElement>) => {
    if (!touchStartRef.current) return;
    const touch = event.touches[0];
    if (!touch) return;
    const deltaX = Math.abs(touch.clientX - touchStartRef.current.x);
    const deltaY = Math.abs(touch.clientY - touchStartRef.current.y);
    if (deltaX > 6 || deltaY > 6) {
      didMoveRef.current = true;
      suppressClickRef.current = true;
      if (longPressTimer) {
        clearTimeout(longPressTimer);
        setLongPressTimer(null);
      }
    }
  };

  const handleTouchEnd = () => {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      setLongPressTimer(null);
    }
    if (!isLongPressTriggered && !didMoveRef.current) {
      onClick();
      suppressClickRef.current = true;
    } else if (didMoveRef.current) {
      suppressClickRef.current = true;
    }
    touchStartRef.current = null;
    didMoveRef.current = false;
  };

  const handleTouchCancel = () => {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      setLongPressTimer(null);
    }
    touchStartRef.current = null;
    didMoveRef.current = false;
    suppressClickRef.current = true;
  };

  const handleClick = () => {
    if (suppressClickRef.current || isLongPressTriggered) {
      suppressClickRef.current = false;
      return;
    }
    if (!isLongPressTriggered) {
      onClick();
    }
  };

  const title = persona?.title ?? "No Persona";
  const description = persona?.description ?? "Continue without a persona";
  const isDefault = persona?.isDefault ?? false;

  return (
    <motion.button
      layout="position"
      initial={false}
      transition={{ duration: 0.12, ease: "easeOut" }}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      onTouchCancel={handleTouchCancel}
      onClick={handleClick}
      className={cn(
        "group relative flex w-full items-center gap-3",
        "rounded-xl",
        "px-3 py-3 text-left",
        interactive.transition.default,
        "active:scale-[0.98]",
        isSelected
          ? "border border-accent/40 bg-accent/10 ring-1 ring-accent/20"
          : "border border-fg/8 bg-fg/3 hover:border-fg/15 hover:bg-fg/6",
      )}
      aria-pressed={isSelected}
    >
      {/* Avatar */}
      <PersonaAvatar persona={persona} size="md" isSelected={isSelected} />

      {/* Content */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              typography.body.size,
              "font-medium truncate",
              isSelected ? "text-accent" : "text-fg",
            )}
          >
            {title}
          </span>
          {isDefault && (
            <span className="inline-flex items-center gap-1 shrink-0 rounded-full border border-info/30 bg-info/10 px-2 py-0.5">
              <Star className="h-2.5 w-2.5 fill-info text-info" />
              <span className="text-[10px] font-medium text-info">Default</span>
            </span>
          )}
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

      {/* Selection indicator */}
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

interface PersonaSelectorProps {
  isOpen: boolean;
  onClose: () => void;
  personas: Persona[];
  selectedPersonaId: string | null | undefined;
  onSelect: (personaId: string | null) => void;
  onLongPress?: (persona: Persona) => void;
  showSearch?: boolean;
  title?: string;
}

export function PersonaSelector({
  isOpen,
  onClose,
  personas,
  selectedPersonaId,
  onSelect,
  onLongPress,
  showSearch = true,
  title = "Select Persona",
}: PersonaSelectorProps) {
  const [searchQuery, setSearchQuery] = useState("");

  const filteredPersonas = useMemo(() => {
    if (!searchQuery.trim()) return personas;
    const q = searchQuery.toLowerCase();
    return personas.filter(
      (p) => p.title.toLowerCase().includes(q) || p.description.toLowerCase().includes(q),
    );
  }, [personas, searchQuery]);

  // Sort personas: default first, then alphabetically
  const sortedPersonas = useMemo(() => {
    return [...filteredPersonas].sort((a, b) => {
      if (a.isDefault && !b.isDefault) return -1;
      if (!a.isDefault && b.isDefault) return 1;
      return a.title.localeCompare(b.title);
    });
  }, [filteredPersonas]);

  const handleSelect = (personaId: string | null) => {
    onSelect(personaId);
    onClose();
    setSearchQuery("");
  };

  const handleClose = () => {
    onClose();
    setSearchQuery("");
  };

  const showSearchBar = showSearch && personas.length > 3;

  return (
    <BottomMenu isOpen={isOpen} onClose={handleClose} title={title}>
      <div className={spacing.group}>
        {/* Empty State */}
        {personas.length === 0 ? (
          <div className={cn("rounded-xl", "border border-warning/20 bg-warning/10 px-5 py-4")}>
            <div className="flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-warning/30 bg-warning/10">
                <User className="h-4 w-4 text-warning" />
              </div>
              <div>
                <p className={cn(typography.body.size, "font-medium text-warning")}>
                  No personas available
                </p>
                <p className={cn(typography.caption.size, "mt-1 text-warning/60")}>
                  Create a persona in settings to personalize your conversations.
                </p>
              </div>
            </div>
          </div>
        ) : (
          <MenuSection>
            {/* Search Bar */}
            {showSearchBar && (
              <div className="relative mb-3">
                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search personas..."
                  className={cn(
                    "w-full rounded-xl border border-fg/10 bg-surface-el/30",
                    "px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40",
                    "focus:border-fg/20 focus:outline-none",
                    interactive.transition.default,
                  )}
                />
              </div>
            )}

            {/* Persona List */}
            <div className="space-y-2 max-h-[50vh] overflow-y-auto -mx-1 px-1">
              <AnimatePresence initial={false}>
                {/* No Persona Option */}
                <PersonaOptionItem
                  key="no-persona"
                  persona={null}
                  isSelected={
                    selectedPersonaId === null ||
                    selectedPersonaId === undefined ||
                    selectedPersonaId === ""
                  }
                  onClick={() => handleSelect(null)}
                />

                {/* Personas */}
                {sortedPersonas.map((persona) => (
                  <PersonaOptionItem
                    key={persona.id}
                    persona={persona}
                    isSelected={persona.id === selectedPersonaId}
                    onClick={() => handleSelect(persona.id)}
                    onLongPress={onLongPress ? () => onLongPress(persona) : undefined}
                  />
                ))}
              </AnimatePresence>

              {/* No Results */}
              {searchQuery && filteredPersonas.length === 0 && (
                <div className="py-8 text-center">
                  <User className="mx-auto h-8 w-8 text-fg/20" />
                  <p className={cn(typography.body.size, "mt-3 text-fg/50")}>
                    No personas found
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
