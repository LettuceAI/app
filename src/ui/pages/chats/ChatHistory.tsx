import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowLeft, Trash2, MessageCircle, AlertCircle, Edit3, Search, X } from "lucide-react";
import { useLocation, useParams } from "react-router-dom";

import type { Character } from "../../../core/storage/schemas";
import {
  listCharacters,
  listSessionPreviews,
  deleteSession,
  updateSessionTitle,
} from "../../../core/storage";
import { typography, radius, cn, colors, interactive } from "../../design-tokens";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuDivider } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";

interface SessionPreview {
  id: string;
  title: string;
  updatedAt: number;
  lastMessage: string;
  archived: boolean;
}

export function ChatHistoryPage() {
  const { characterId } = useParams<{ characterId: string }>();
  const location = useLocation();
  const { go, backOrReplace } = useNavigationManager();
  const [character, setCharacter] = useState<Character | null>(null);
  const [sessions, setSessions] = useState<SessionPreview[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const [deleteTarget, setDeleteTarget] = useState<SessionPreview | null>(null);
  const [query, setQuery] = useState(() => {
    const storageKey = characterId ? `chatHistoryQuery:${characterId}` : "chatHistoryQuery";
    const fromStorage = sessionStorage.getItem(storageKey);
    if (fromStorage != null) return fromStorage;
    return new URLSearchParams(window.location.search).get("q") ?? "";
  });

  useEffect(() => {
    const storageKey = characterId ? `chatHistoryQuery:${characterId}` : "chatHistoryQuery";
    if (query.trim()) sessionStorage.setItem(storageKey, query);
    else sessionStorage.removeItem(storageKey);
  }, [characterId, query]);

  useEffect(() => {
    // Keep URL shareable without triggering react-router navigation/remounts.
    const handle = window.setTimeout(() => {
      const url = new URL(window.location.href);
      const next = query.trim();
      if (next) url.searchParams.set("q", next);
      else url.searchParams.delete("q");
      window.history.replaceState(window.history.state, "", url.toString());
    }, 150);
    return () => window.clearTimeout(handle);
  }, [query]);

  // Load data
  useEffect(() => {
    const loadData = async () => {
      if (!characterId) return;

      try {
        setIsLoading(true);
        setError(null);

        // Load character
        const characters = await listCharacters();
        const char = characters.find((c) => c.id === characterId);
        setCharacter(char || null);

        const previews = await listSessionPreviews(characterId);
        setSessions(
          previews.map((p) => {
            const lastMessage = p.lastMessage || "";
            return {
              id: p.id,
              title: p.title?.trim() ? p.title : "Untitled Chat",
              updatedAt: p.updatedAt,
              lastMessage: lastMessage.slice(0, 100) + (lastMessage.length > 100 ? "..." : ""),
              archived: p.archived,
            };
          }),
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load data");
      } finally {
        setIsLoading(false);
      }
    };

    loadData();
  }, [characterId]);

  const handleDelete = useCallback(async (sessionId: string) => {
    setBusyIds((prev) => new Set(prev).add(sessionId));
    try {
      await deleteSession(sessionId);
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
    } catch (err) {
      setError(`Failed to delete: ${err}`);
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
  }, []);

  const handleRename = useCallback(async (sessionId: string, newTitle: string) => {
    setBusyIds((prev) => new Set(prev).add(sessionId));
    try {
      await updateSessionTitle(sessionId, newTitle);
      setSessions((prev) => prev.map((s) => (s.id === sessionId ? { ...s, title: newTitle } : s)));
    } catch (err) {
      setError(`Failed to rename: ${err}`);
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
  }, []);

  const filteredSessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => {
      if (s.title.toLowerCase().includes(q)) return true;
      if (s.lastMessage.toLowerCase().includes(q)) return true;
      return false;
    });
  }, [query, sessions]);

  const sessionId = useMemo(() => {
    return new URLSearchParams(location.search).get("sessionId");
  }, [location.search]);

  if (!characterId) {
    return (
      <div className="min-h-screen bg-surface flex items-center justify-center">
        <div className="text-center">
          <MessageCircle className="mx-auto mb-4 h-12 w-12 text-white/30" />
          <p className={cn(typography.body.size, "text-white/60")}>Character not found</p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="min-h-screen bg-surface flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border border-white/10 border-t-white/60" />
      </div>
    );
  }

  return (
    <div className={cn("flex min-h-screen flex-col", colors.surface.base, colors.text.primary)}>
      {/* Header */}
      <header
        className={cn(
          "z-20 shrink-0 border-b border-white/10 px-4 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] sticky top-0",
          "bg-surface",
        )}
      >
        <div className="flex items-center gap-3">
          <div className="flex flex-1 items-center min-w-0">
            <button
              onClick={() =>
                backOrReplace(
                  characterId ? Routes.chatSettingsSession(characterId, sessionId) : Routes.chat,
                )
              }
              className="flex shrink-0 px-[0.6em] py-[0.3em] items-center justify-center -ml-2 text-white transition hover:text-white/80"
              aria-label="Back to chat settings"
            >
              <ArrowLeft size={14} strokeWidth={2.5} />
            </button>
            <div className="min-w-0 flex-1 text-left">
              <p className="truncate text-xl font-bold text-white/90">Chat History</p>
              <p className="mt-0.5 truncate text-xs text-white/50">
                {character
                  ? `Previous conversations with ${character.name}`
                  : "Previous conversations"}
              </p>
            </div>
          </div>
        </div>
      </header>

      {/* Content */}
      <main className="flex-1 overflow-y-auto px-3 pt-4">
        {error && (
          <div
            className={cn("mb-4 p-4 border border-red-400/30 bg-red-400/10 text-center", radius.lg)}
          >
            <AlertCircle className="mx-auto mb-2 h-5 w-5 text-red-400" />
            <p className={cn(typography.bodySmall.size, "text-red-200")}>{error}</p>
            <button
              onClick={() => window.location.reload()}
              className={cn(
                "mt-3 px-4 py-2 border border-red-400/40 bg-red-400/20 text-red-100",
                radius.md,
                "active:scale-95 transition-transform",
              )}
            >
              Retry
            </button>
          </div>
        )}

        {sessions.length > 0 && (
          <div className="mb-4">
            <div className={cn("relative")}>
              <Search
                className={cn(
                  "absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4",
                  colors.text.tertiary,
                )}
              />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search chats..."
                className={cn(
                  "w-full pl-10 pr-10 py-2.5",
                  "border bg-white/5",
                  colors.border.subtle,
                  radius.lg,
                  typography.bodySmall.size,
                  "text-white placeholder-white/40",
                  "focus:outline-none focus:ring-2 focus:ring-white/10 focus:border-white/20",
                )}
              />
              {query.trim().length > 0 && (
                <button
                  type="button"
                  onClick={() => setQuery("")}
                  className={cn(
                    "absolute right-2 top-1/2 -translate-y-1/2",
                    "flex items-center justify-center",
                    radius.full,
                    colors.text.tertiary,
                    "hover:text-white",
                    interactive.transition.fast,
                    interactive.active.scale,
                  )}
                  aria-label="Clear search"
                >
                  <X size={16} />
                </button>
              )}
            </div>
            {query.trim() ? (
              <p className={cn(typography.caption.size, colors.text.tertiary, "mt-2")}>
                {filteredSessions.length.toLocaleString()} result
                {filteredSessions.length === 1 ? "" : "s"}
              </p>
            ) : null}
          </div>
        )}

        {sessions.length === 0 ? (
          <div className="text-center py-20">
            <MessageCircle className="mx-auto mb-4 h-12 w-12 text-white/30" />
            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-white/70 mb-2")}>
              No conversations yet
            </h3>
            <p className={cn(typography.bodySmall.size, "text-white/40 mb-6")}>
              Start chatting to see your history here
            </p>
          </div>
        ) : filteredSessions.length === 0 ? (
          <div className="text-center py-20">
            <MessageCircle className="mx-auto mb-4 h-12 w-12 text-white/30" />
            <h3 className={cn(typography.h3.size, typography.h3.weight, "text-white/70 mb-2")}>
              No matching chats
            </h3>
            <p className={cn(typography.bodySmall.size, "text-white/40 mb-6")}>
              Try a different search term
            </p>
            <button
              type="button"
              onClick={() => setQuery("")}
              className={cn(
                "px-4 py-2 border bg-white/5 text-white/80",
                colors.border.subtle,
                radius.md,
                typography.bodySmall.size,
                interactive.active.scale,
                interactive.hover.brightness,
                interactive.transition.fast,
              )}
            >
              Clear search
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            {filteredSessions.map((session) => (
              <SessionCard
                key={session.id}
                session={session}
                onSelect={() => go(Routes.chatSession(characterId!, session.id))}
                onDelete={() => setDeleteTarget(session)}
                onRename={(newTitle) => handleRename(session.id, newTitle)}
                isBusy={busyIds.has(session.id)}
              />
            ))}
          </div>
        )}
      </main>

      <BottomMenu
        isOpen={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title="Delete chat?"
        includeExitIcon={false}
      >
        <div className="rounded-xl border border-white/10 bg-white/4 p-3">
          <p className={cn(typography.bodySmall.size, "font-semibold text-white/90 truncate")}>
            {deleteTarget?.title || "Untitled Chat"}
          </p>
          {deleteTarget ? (
            <p className={cn(typography.caption.size, "text-white/45 mt-0.5")}>
              {formatTimeAgo(deleteTarget.updatedAt)}
            </p>
          ) : null}
          {deleteTarget?.lastMessage ? (
            <p className={cn(typography.bodySmall.size, "text-white/60 mt-2 line-clamp-2")}>
              {deleteTarget.lastMessage}
            </p>
          ) : null}
        </div>

        <MenuDivider />

        <MenuButtonGroup>
          <MenuButton
            icon={Trash2}
            title={deleteTarget && busyIds.has(deleteTarget.id) ? "Deleting..." : "Delete chat"}
            description="Permanently removes it from history"
            color="from-rose-500 to-red-600"
            disabled={!deleteTarget || busyIds.has(deleteTarget.id)}
            onClick={() => {
              if (!deleteTarget) return;
              void (async () => {
                await handleDelete(deleteTarget.id);
                setDeleteTarget(null);
              })();
            }}
          />
          <MenuButton
            icon={X}
            title="Cancel"
            description="Keep this chat"
            color="from-blue-500 to-blue-600"
            disabled={!!deleteTarget && busyIds.has(deleteTarget.id)}
            onClick={() => setDeleteTarget(null)}
          />
        </MenuButtonGroup>
      </BottomMenu>
    </div>
  );
}

function SessionCard({
  session,
  onSelect,
  onDelete,
  onRename,
  isBusy,
}: {
  session: SessionPreview;
  onSelect: () => void;
  onDelete: () => void;
  onRename: (newTitle: string) => void;
  isBusy: boolean;
}) {
  const [isRenaming, setIsRenaming] = useState(false);
  const [editTitle, setEditTitle] = useState(session.title);

  const handleRenameSubmit = () => {
    if (editTitle.trim() && editTitle !== session.title) {
      onRename(editTitle.trim());
    }
    setIsRenaming(false);
  };

  const handleCancel = () => {
    setEditTitle(session.title);
    setIsRenaming(false);
  };

  return (
    <div
      className={cn(
        "border border-white/10 bg-white/5 overflow-hidden",
        radius.lg,
        session.archived && "border-amber-400/20 bg-amber-400/5",
      )}
    >
      {/* Main content - tap to open */}
      <button
        onClick={onSelect}
        disabled={isBusy || isRenaming}
        className="w-full p-4 text-left disabled:opacity-50 active:bg-white/10 transition-colors"
      >
        <h3 className={cn(typography.h3.size, typography.h3.weight, "text-white mb-2")}>
          {session.title}
        </h3>

        <p className={cn(typography.bodySmall.size, "text-white/50 mb-3")}>
          {formatTimeAgo(session.updatedAt)}
        </p>

        {session.lastMessage && (
          <p
            className={cn(typography.bodySmall.size, "text-white/70 line-clamp-2 leading-relaxed")}
          >
            {session.lastMessage}
          </p>
        )}
      </button>

      {/* Rename input when editing */}
      {isRenaming && (
        <div className="px-4 pb-3 border-t border-white/5 pt-3">
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleRenameSubmit();
              if (e.key === "Escape") handleCancel();
            }}
            autoFocus
            className={cn(
              "w-full px-3 py-2 bg-white/10 border border-white/20 text-white mb-2",
              radius.md,
              typography.body.size,
              "focus:outline-none focus:border-blue-400/60",
            )}
            placeholder="Chat title..."
          />
          <div className="flex gap-2">
            <button
              onClick={handleRenameSubmit}
              disabled={!editTitle.trim()}
              className={cn(
                "flex-1 px-3 py-2 border border-blue-400/40 bg-blue-400/20 text-blue-100",
                radius.md,
                typography.bodySmall.size,
                "active:scale-95 disabled:opacity-50 transition-all",
              )}
            >
              Save
            </button>
            <button
              onClick={handleCancel}
              className={cn(
                "flex-1 px-3 py-2 border border-white/10 bg-white/5 text-white/60",
                radius.md,
                typography.bodySmall.size,
                "active:scale-95 transition-all",
              )}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Action buttons */}
      {!isRenaming && (
        <div className="px-4 pb-3 border-t border-white/5 pt-3 flex gap-2">
          <button
            onClick={() => setIsRenaming(true)}
            disabled={isBusy}
            className={cn(
              "flex items-center gap-2 px-3 py-2 border border-white/10 bg-white/5 text-white/60",
              radius.md,
              typography.bodySmall.size,
              "active:scale-95 active:bg-blue-400/10 active:text-blue-300 active:border-blue-400/40 disabled:opacity-50 transition-all",
            )}
          >
            <Edit3 size={14} />
            Rename
          </button>
          <button
            onClick={onDelete}
            disabled={isBusy}
            className={cn(
              "flex items-center gap-2 px-3 py-2 border border-white/10 bg-white/5 text-white/60",
              radius.md,
              typography.bodySmall.size,
              "active:scale-95 active:bg-red-400/10 active:text-red-300 active:border-red-400/40 disabled:opacity-50 transition-all",
            )}
          >
            <Trash2 size={14} />
            Delete
          </button>
        </div>
      )}
    </div>
  );
}

function formatTimeAgo(timestamp: number): string {
  const diffMs = Date.now() - timestamp;
  const diffMinutes = Math.floor(diffMs / (1000 * 60));

  if (diffMinutes < 1) return "Just now";
  if (diffMinutes < 60) return `${diffMinutes}m ago`;

  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours}h ago`;

  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays}d ago`;

  return new Date(timestamp).toLocaleDateString();
}

export default ChatHistoryPage;
