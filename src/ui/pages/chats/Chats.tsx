import { useEffect, useState, memo, useRef } from "react";
import { Edit2, Trash2, Download, EyeOff, Paintbrush } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { motion } from "framer-motion";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

import {
  listCharacters,
  createSession,
  listSessionPreviews,
  archiveSession,
  SESSION_UPDATED_EVENT,
  deleteCharacter,
} from "../../../core/storage/repo";
import type { Character } from "../../../core/storage/schemas";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";
import { BottomMenu, CharacterExportMenu } from "../../components";
import { AvatarImage } from "../../components/AvatarImage";
import { useAvatar } from "../../hooks/useAvatar";
import { useAvatarGradient } from "../../hooks/useAvatarGradient";
import {
  exportCharacterWithFormat,
  downloadJson,
  generateExportFilenameWithFormat,
  type CharacterFileFormat,
} from "../../../core/storage/characterTransfer";

export function ChatPage() {
  const [characters, setCharacters] = useState<Character[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedCharacter, setSelectedCharacter] = useState<Character | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportMenuOpen, setExportMenuOpen] = useState(false);
  const [exportTarget, setExportTarget] = useState<Character | null>(null);
  const [latestSessionByCharacter, setLatestSessionByCharacter] = useState<
    Record<string, { id: string; updatedAt: number; archived: boolean }>
  >({});
  const [hiding, setHiding] = useState(false);
  const navigate = useNavigate();

  const loadCharacters = async () => {
    try {
      const [list, previews] = await Promise.all([
        listCharacters(),
        listSessionPreviews().catch(() => []),
      ]);
      const latestByCharacter: Record<
        string,
        { id: string; updatedAt: number; archived: boolean }
      > = {};
      const charactersWithSessions = new Set<string>();

      previews.forEach((preview) => {
        charactersWithSessions.add(preview.characterId);
        const current = latestByCharacter[preview.characterId];
        if (!current || preview.updatedAt > current.updatedAt) {
          latestByCharacter[preview.characterId] = {
            id: preview.id,
            updatedAt: preview.updatedAt,
            archived: preview.archived,
          };
        }
      });

      const visible = list.filter((character) => {
        if (!charactersWithSessions.has(character.id)) return true;
        return !latestByCharacter[character.id]?.archived;
      });

      const sorted = [...visible].sort((a, b) => {
        const aTime = latestByCharacter[a.id]?.updatedAt ?? a.updatedAt ?? a.createdAt ?? 0;
        const bTime = latestByCharacter[b.id]?.updatedAt ?? b.updatedAt ?? b.createdAt ?? 0;
        return bTime - aTime;
      });

      setLatestSessionByCharacter(latestByCharacter);
      setCharacters(sorted);
    } catch (err) {
      console.error("Failed to load characters:", err);
    }
  };

  useEffect(() => {
    (async () => {
      try {
        await loadCharacters();
      } finally {
        setLoading(false);
      }
    })();

    // Listen for database reload events to refresh data
    let unlisten: UnlistenFn | null = null;
    (async () => {
      unlisten = await listen("database-reloaded", () => {
        console.log("Database reloaded, refreshing characters...");
        loadCharacters();
      });
    })();

    const handleSessionUpdated = () => {
      loadCharacters();
    };
    window.addEventListener(SESSION_UPDATED_EVENT, handleSessionUpdated);

    return () => {
      if (unlisten) unlisten();
      window.removeEventListener(SESSION_UPDATED_EVENT, handleSessionUpdated);
    };
  }, []);

  const startChat = async (character: Character) => {
    try {
      const latestSessionId = latestSessionByCharacter[character.id]?.id;
      if (latestSessionId) {
        navigate(`/chat/${character.id}?sessionId=${latestSessionId}`);
        return;
      }

      const session = await createSession(
        character.id,
        "New Chat",
        character.scenes && character.scenes.length > 0 ? character.scenes[0].id : undefined,
      );
      navigate(`/chat/${character.id}?sessionId=${session.id}`);
    } catch (error) {
      console.error("Failed to load or create session:", error);
      navigate(`/chat/${character.id}`);
    }
  };

  const handleEditCharacter = (character: Character) => {
    navigate(`/settings/characters/${character.id}/edit`);
  };

  const handleDelete = async () => {
    if (!selectedCharacter) return;

    try {
      setDeleting(true);
      await deleteCharacter(selectedCharacter.id);
      await loadCharacters();
      setShowDeleteConfirm(false);
      setSelectedCharacter(null);
    } catch (err) {
      console.error("Failed to delete character:", err);
    } finally {
      setDeleting(false);
    }
  };

  const handleExport = () => {
    if (!selectedCharacter) return;
    setExportTarget(selectedCharacter);
    setSelectedCharacter(null);
    setExportMenuOpen(true);
  };

  const handleHide = async () => {
    if (!selectedCharacter) return;
    const latestSessionId = latestSessionByCharacter[selectedCharacter.id]?.id;
    if (!latestSessionId) {
      setSelectedCharacter(null);
      return;
    }

    try {
      setHiding(true);
      await archiveSession(latestSessionId, true);
      await loadCharacters();
      setSelectedCharacter(null);
    } catch (err) {
      console.error("Failed to hide character session:", err);
    } finally {
      setHiding(false);
    }
  };

  const handleExportFormat = async (format: CharacterFileFormat) => {
    if (!exportTarget) return;

    try {
      setExporting(true);
      const exportJson = await exportCharacterWithFormat(exportTarget.id, format);
      const filename = generateExportFilenameWithFormat(exportTarget.name, format);
      await downloadJson(exportJson, filename);
    } catch (err) {
      console.error("Failed to export character:", err);
    } finally {
      setExporting(false);
      setExportMenuOpen(false);
      setExportTarget(null);
    }
  };

  return (
    <div className="flex h-full flex-col pb-6 text-gray-200">
      <main className="flex-1 overflow-y-auto px-1 lg:px-8 pt-4 mx-auto w-full max-w-md lg:max-w-none">
        {loading ? (
          <CharacterSkeleton />
        ) : characters.length ? (
          <CharacterList
            characters={characters}
            onSelect={startChat}
            onLongPress={setSelectedCharacter}
          />
        ) : (
          <EmptyState />
        )}
      </main>

      {/* Character Actions Menu */}
      <BottomMenu
        isOpen={Boolean(selectedCharacter)}
        onClose={() => setSelectedCharacter(null)}
        includeExitIcon={false}
        title={selectedCharacter?.name || ""}
      >
        {selectedCharacter && (
          <div className="space-y-2">
            <button
              onClick={() => handleEditCharacter(selectedCharacter)}
              className="flex w-full items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-left transition hover:border-white/20 hover:bg-white/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-white/10">
                <Edit2 className="h-4 w-4 text-white/70" />
              </div>
              <span className="text-sm font-medium text-white">Edit Character</span>
            </button>

            <button
              onClick={handleExport}
              disabled={exporting}
              className="flex w-full items-center gap-3 rounded-xl border border-blue-400/30 bg-blue-400/10 px-4 py-3 text-left transition hover:border-blue-400/50 hover:bg-blue-400/20 disabled:opacity-50"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-blue-400/30 bg-blue-400/20">
                <Download className="h-4 w-4 text-blue-400" />
              </div>
              <span className="text-sm font-medium text-blue-300">
                {exporting ? "Exporting..." : "Export Character"}
              </span>
            </button>

            <button
              onClick={() => {
                const charId = selectedCharacter.id;
                setSelectedCharacter(null);
                navigate(`/settings/accessibility/chat?characterId=${charId}`);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-purple-400/30 bg-purple-400/10 px-4 py-3 text-left transition hover:border-purple-400/50 hover:bg-purple-400/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-purple-400/30 bg-purple-400/20">
                <Paintbrush className="h-4 w-4 text-purple-400" />
              </div>
              <span className="text-sm font-medium text-purple-300">Chat Appearance</span>
            </button>

            <button
              onClick={handleHide}
              disabled={hiding || !latestSessionByCharacter[selectedCharacter.id]}
              className="flex w-full items-center gap-3 rounded-xl border border-amber-400/30 bg-amber-400/10 px-4 py-3 text-left transition hover:border-amber-400/50 hover:bg-amber-400/20 disabled:opacity-50"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-amber-400/30 bg-amber-400/20">
                <EyeOff className="h-4 w-4 text-amber-400" />
              </div>
              <span className="text-sm font-medium text-amber-200">
                {hiding ? "Hiding..." : "Hide this character"}
              </span>
            </button>

            <button
              onClick={() => {
                setShowDeleteConfirm(true);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-left transition hover:border-red-500/50 hover:bg-red-500/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-red-500/30 bg-red-500/20">
                <Trash2 className="h-4 w-4 text-red-400" />
              </div>
              <span className="text-sm font-medium text-red-300">Delete Character</span>
            </button>
          </div>
        )}
      </BottomMenu>

      <CharacterExportMenu
        isOpen={exportMenuOpen}
        onClose={() => {
          setExportMenuOpen(false);
          setExportTarget(null);
        }}
        onSelect={handleExportFormat}
        exporting={exporting}
      />

      {/* Delete Confirmation */}
      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title="Delete Character?"
      >
        <div className="space-y-4">
          <p className="text-sm text-white/70">
            Are you sure you want to delete "{selectedCharacter?.name}"? This will also delete all
            chat sessions with this character.
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              disabled={deleting}
              className="flex-1 rounded-xl border border-white/10 bg-white/5 py-3 text-sm font-medium text-white transition hover:border-white/20 hover:bg-white/10 disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleDelete}
              disabled={deleting}
              className="flex-1 rounded-xl border border-red-500/30 bg-red-500/20 py-3 text-sm font-medium text-red-300 transition hover:bg-red-500/30 disabled:opacity-50"
            >
              {deleting ? "Deleting..." : "Delete"}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

function CharacterList({
  characters,
  onSelect,
  onLongPress,
}: {
  characters: Character[];
  onSelect: (character: Character) => void | Promise<void>;
  onLongPress: (character: Character) => void;
}) {
  const [visibleCount, setVisibleCount] = useState(10);

  useEffect(() => {
    if (visibleCount < characters.length) {
      const timer = setTimeout(() => {
        setVisibleCount((prev) => Math.min(prev + 10, characters.length));
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [visibleCount, characters.length]);

  useEffect(() => {
    setVisibleCount(10);
  }, [characters]);

  return (
    <div className="space-y-2 lg:space-y-3 pb-24">
      {characters.slice(0, visibleCount).map((character) => (
        <CharacterCard
          key={character.id}
          character={character}
          onSelect={onSelect}
          onLongPress={onLongPress}
        />
      ))}
    </div>
  );
}

function CharacterSkeleton() {
  return (
    <div className={spacing.item}>
      {[0, 1, 2].map((index) => (
        <div
          key={index}
          className={cn(
            "h-16 animate-pulse p-2 pr-4",
            "rounded-full",
            "border border-white/5 bg-white/5",
          )}
        >
          <div className="flex items-center gap-3">
            <div className="h-12 w-12 rounded-full bg-white/10" />
            <div className="flex-1 space-y-2">
              <div className="h-3.5 w-1/3 rounded-full bg-white/10" />
              <div className="h-3 w-2/3 rounded-full bg-white/5" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function EmptyState() {
  return (
    <div
      className={cn(
        "p-8 text-center",
        radius.lg,
        "border border-dashed border-white/10 bg-white/2",
      )}
    >
      <div className={spacing.field}>
        <h3 className={cn(typography.h3.size, typography.h3.weight, "text-white")}>
          No characters yet
        </h3>
        <p className={cn(typography.body.size, typography.body.lineHeight, "text-white/50")}>
          Create your first character from the + button below to start chatting
        </p>
      </div>
    </div>
  );
}

function isImageLike(s?: string) {
  if (!s) return false;
  const lower = s.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
}

const CharacterAvatar = memo(
  ({ character, className }: { character: Character; className?: string }) => {
    const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

    if (avatarUrl && isImageLike(avatarUrl)) {
      return (
        <AvatarImage
          src={avatarUrl}
          alt={`${character.name} avatar`}
          crop={character.avatarCrop}
          applyCrop
          className={className}
        />
      );
    }

    // Fallback: initials with a subtle gradient
    const initials = character.name.slice(0, 2).toUpperCase();
    return (
      <div
        className={cn(
          "flex h-full w-full items-center justify-center",
          "bg-linear-to-br from-white/20 to-white/5",
          className,
        )}
      >
        <span className="text-lg font-bold text-white/80">{initials}</span>
      </div>
    );
  },
);

CharacterAvatar.displayName = "CharacterAvatar";

const CharacterCard = memo(
  ({
    character,
    onSelect,
    onLongPress,
  }: {
    character: Character;
    onSelect: (character: Character) => void;
    onLongPress: (character: Character) => void;
  }) => {
    const descriptionPreview =
      (character.description || character.definition || "").trim() || "No description yet";
    const { gradientCss, hasGradient, textColor, textSecondary } = useAvatarGradient(
      "character",
      character.id,
      character.avatarPath,
      character.disableAvatarGradient,
      // Pass custom colors if enabled
      character.customGradientEnabled && character.customGradientColors?.length
        ? {
            colors: character.customGradientColors,
            textColor: character.customTextColor,
            textSecondary: character.customTextSecondary,
          }
        : undefined,
    );
    // Long-press support for desktop
    const longPressTimer = useRef<number | null>(null);
    const isLongPress = useRef(false);

    const handlePointerDown = () => {
      isLongPress.current = false;
      longPressTimer.current = window.setTimeout(() => {
        isLongPress.current = true;
        onLongPress(character);
      }, 500);
    };

    const handlePointerUp = () => {
      if (longPressTimer.current) {
        clearTimeout(longPressTimer.current);
        longPressTimer.current = null;
      }
    };

    const handlePointerLeave = () => {
      if (longPressTimer.current) {
        clearTimeout(longPressTimer.current);
        longPressTimer.current = null;
      }
    };

    const handleClick = () => {
      // Don't trigger click if it was a long press
      if (isLongPress.current) {
        isLongPress.current = false;
        return;
      }
      onSelect(character);
    };

    const handleContextMenu = (e: React.MouseEvent) => {
      e.preventDefault();
      onLongPress(character);
    };

    return (
      <motion.button
        layoutId={`character-${character.id}`}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        onPointerDown={handlePointerDown}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerLeave}
        className={cn(
          "group relative flex w-full items-center gap-3.5 lg:gap-6 p-3.5 lg:p-6 text-left",
          "rounded-2xl lg:rounded-3xl",
          interactive.transition.default,
          interactive.active.scale,
          hasGradient ? "" : "bg-[#1a1b23] hover:bg-[#22232d]",
        )}
        style={hasGradient ? { background: gradientCss } : {}}
      >
        {/* Circular Avatar */}
        <div
          className={cn(
            "relative h-14 w-14 lg:h-24 lg:w-24 shrink-0 overflow-hidden rounded-full",
            hasGradient ? "ring-2 ring-white/20" : "ring-1 ring-white/10",
            "shadow-lg",
          )}
        >
          <CharacterAvatar character={character} />
        </div>

        {/* Content */}
        <div className="flex  min-w-0 flex-1 flex-col gap-0.5 lg:gap-1.5 py-1">
          <h3
            className={cn(
              "truncate font-semibold text-[15px] lg:text-xl leading-tight",
              hasGradient ? "" : "text-white",
            )}
            style={hasGradient ? { color: textColor } : {}}
          >
            {character.name}
          </h3>
          <p
            className={cn(
              "line-clamp-1 lg:line-clamp-2 text-[13px] lg:text-base leading-tight lg:leading-relaxed",
              hasGradient ? "" : "text-white/50",
            )}
            style={hasGradient ? { color: textSecondary } : {}}
          >
            {descriptionPreview}
          </p>
        </div>

        {/* chevron */}
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={cn(
            "shrink-0 transition-all",
            hasGradient ? "" : "text-white/30 group-hover:text-white/60",
          )}
          style={hasGradient ? { color: textSecondary } : {}}
        >
          <path d="m9 18 6-6-6-6" />
        </svg>
      </motion.button>
    );
  },
);

CharacterCard.displayName = "CharacterCard";
