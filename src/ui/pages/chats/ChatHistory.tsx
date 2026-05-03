import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowLeft,
  ChevronLeft,
  ChevronRight,
  Trash2,
  MessageCircle,
  AlertCircle,
  Edit3,
  Search,
  X,
  Download,
  User,
} from "lucide-react";
import { useLocation, useParams } from "react-router-dom";

import type { Character } from "../../../core/storage/schemas";
import {
  listCharacters,
  listSessionPreviews,
  deleteSession,
  updateSessionTitle,
} from "../../../core/storage";
import { storageBridge } from "../../../core/storage/files";
import { typography, radius, cn, colors, interactive } from "../../design-tokens";
import { WindowControlButtons, useDragRegionProps, hasCustomWindowControls } from "../../components/App/TopNav";
import { BottomMenu, MenuButton, MenuButtonGroup, MenuDivider } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";
import { useI18n } from "../../../core/i18n/context";

interface SessionPreview {
  id: string;
  title: string;
  updatedAt: number;
  lastMessage: string;
  messageCount: number;
  archived: boolean;
}

export function ChatHistoryPage() {
  const { characterId } = useParams<{ characterId: string }>();
  const location = useLocation();
  const { go, backOrReplace } = useNavigationManager();
  const { t } = useI18n();
  const dragRegionProps = useDragRegionProps();
  const formatTimeAgo = useFormatTimeAgo();
  const [character, setCharacter] = useState<Character | null>(null);
  const [sessions, setSessions] = useState<SessionPreview[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const [deleteTarget, setDeleteTarget] = useState<SessionPreview | null>(null);
  const [renameTarget, setRenameTarget] = useState<SessionPreview | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [exportTarget, setExportTarget] = useState<SessionPreview | null>(null);
  const [exporting, setExporting] = useState(false);
  const [groupPages, setGroupPages] = useState<Record<string, number>>({});
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

  useEffect(() => {
    setRenameDraft(renameTarget?.title ?? "");
  }, [renameTarget]);

  useEffect(() => {
    setGroupPages({});
  }, [characterId, query]);

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
              title: p.title?.trim() ? p.title : t("chats.untitledChat"),
              updatedAt: p.updatedAt,
              lastMessage: lastMessage.slice(0, 100) + (lastMessage.length > 100 ? "..." : ""),
              messageCount: p.messageCount,
              archived: p.archived,
            };
          }),
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : t("chats.history.failedLoad"));
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
      setError(t("chats.history.failedDelete", { error: String(err) }));
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
      setError(t("chats.history.failedRename", { error: String(err) }));
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
    }
  }, []);

  const handleExportChatpkg = useCallback(
    async (includeCharacterId: boolean) => {
      if (!exportTarget) return;
      try {
        setExporting(true);
        const path = await storageBridge.chatpkgExportSingleChat(
          exportTarget.id,
          includeCharacterId,
        );
        setExportTarget(null);
        alert(t("chats.history.chatPackageExportedTo", { path }));
      } catch (err) {
        console.error("Failed to export chat package:", err);
        alert(typeof err === "string" ? err : t("chats.history.failedExportChatPackage"));
      } finally {
        setExporting(false);
      }
    },
    [exportTarget],
  );

  const handleExportSillyTavern = useCallback(async () => {
    if (!exportTarget) return;
    try {
      setExporting(true);
      const path = await storageBridge.chatpkgExportSingleChatSillyTavern(exportTarget.id);
      setExportTarget(null);
      alert(t("chats.history.sillyTavernExportedTo", { path }));
    } catch (err) {
      console.error("Failed to export SillyTavern chat:", err);
      alert(typeof err === "string" ? err : t("chats.history.failedExportSillyTavern"));
    } finally {
      setExporting(false);
    }
  }, [exportTarget]);

  const filteredSessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => {
      if (s.title.toLowerCase().includes(q)) return true;
      if (s.lastMessage.toLowerCase().includes(q)) return true;
      return false;
    });
  }, [query, sessions]);

  const groupedSessions = useMemo(() => {
    const today: SessionPreview[] = [];
    const yesterday: SessionPreview[] = [];
    const last7Days: SessionPreview[] = [];
    const older: SessionPreview[] = [];

    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    const yesterdayStart = todayStart - 24 * 60 * 60 * 1000;
    const sevenDaysAgo = todayStart - 7 * 24 * 60 * 60 * 1000;

    filteredSessions.forEach((s) => {
      if (s.updatedAt >= todayStart) today.push(s);
      else if (s.updatedAt >= yesterdayStart) yesterday.push(s);
      else if (s.updatedAt >= sevenDaysAgo) last7Days.push(s);
      else older.push(s);
    });

    return [
      { key: "today", label: t("common.time.today"), sessions: today },
      { key: "yesterday", label: t("common.time.yesterday"), sessions: yesterday },
      { key: "last7Days", label: t("common.time.last7Days"), sessions: last7Days },
      { key: "older", label: t("common.time.older"), sessions: older },
    ].filter((g) => g.sessions.length > 0);
  }, [filteredSessions, t]);

  const sessionId = useMemo(() => {
    return new URLSearchParams(location.search).get("sessionId");
  }, [location.search]);

  if (!characterId) {
    return (
      <div className="min-h-screen bg-surface flex items-center justify-center">
        <div className="text-center">
          <MessageCircle className="mx-auto mb-4 h-12 w-12 text-fg/30" />
          <p className={cn(typography.body.size, "text-fg/60")}>{t("chats.characterNotFound")}</p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="min-h-screen bg-surface flex items-center justify-center">
        <div className="animate-spin rounded-full h-8 w-8 border border-fg/10 border-t-white/60" />
      </div>
    );
  }

  return (
    <div className={cn("flex h-full flex-col", colors.surface.base, colors.text.primary)}>
      <header
        className={cn("z-20 shrink-0 border-b border-fg/10 bg-surface pl-4 pb-3", hasCustomWindowControls ? "pr-0" : "pr-4")}
        style={{
          paddingTop: "calc(env(safe-area-inset-top) + 12px)",
        }}
        {...dragRegionProps}
      >
        <div className="flex items-center justify-between gap-3" {...dragRegionProps}>
          <div className="flex min-w-0 items-center">
            <button
              onClick={() =>
                backOrReplace(
                  characterId ? Routes.chatSettingsSession(characterId, sessionId) : Routes.chat,
                )
              }
              className={cn(
                "flex shrink-0 items-center justify-center -ml-2 px-[0.6em] py-[0.3em]",
                colors.text.primary,
                interactive.transition.fast,
                "hover:text-fg/80",
              )}
              aria-label={t("chats.header.back")}
            >
              <ArrowLeft size={18} strokeWidth={2.5} />
            </button>
            <div className="min-w-0 text-left">
              <p className="truncate text-xl font-bold text-fg/90">{t("chats.chatHistory")}</p>
              <p className="mt-0.5 truncate text-xs text-fg/50">
                {character
                  ? t("chats.previousConversationsWithCharacter", { name: character.name })
                  : t("chats.previousConversations")}
              </p>
            </div>
          </div>
          <WindowControlButtons />
        </div>
      </header>

      <main className="flex-1 overflow-y-auto px-3 pt-4">
        <div className="mx-auto w-full max-w-5xl">
          {sessions.length > 0 && (
            <div className="mb-4">
              <div className="relative">
                <Search
                  className={cn(
                    "absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4",
                    colors.text.tertiary,
                  )}
                />
                <input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder={t("chats.searchChats")}
                  className={cn(
                    "w-full pl-10 pr-10 py-2.5",
                    "border bg-fg/5",
                    colors.border.subtle,
                    radius.lg,
                    typography.bodySmall.size,
                    "text-fg placeholder-fg/40",
                    "focus:outline-none focus:ring-2 focus:ring-white/10 focus:border-fg/20",
                  )}
                />
                {query.trim().length > 0 && (
                  <button
                    type="button"
                    onClick={() => setQuery("")}
                    className={cn(
                      "absolute right-2 top-1/2 -translate-y-1/2 flex items-center justify-center",
                      radius.full,
                      colors.text.tertiary,
                      "hover:text-fg",
                      interactive.transition.fast,
                      interactive.active.scale,
                    )}
                    aria-label={t("common.buttons.clearSearch")}
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

          {error && (
            <div
              className={cn("mb-4 border border-danger/30 bg-danger/10 p-4 text-center", radius.lg)}
            >
              <AlertCircle className="mx-auto mb-2 h-5 w-5 text-danger" />
              <p className={cn(typography.bodySmall.size, "text-danger")}>{error}</p>
              <button
                onClick={() => window.location.reload()}
                className={cn(
                  "mt-3 px-4 py-2 border border-danger/40 bg-danger/20 text-danger",
                  radius.md,
                  "active:scale-95 transition-transform",
                )}
              >
                {t("common.buttons.retry")}
              </button>
            </div>
          )}

          {sessions.length === 0 ? (
            <div className="py-20 text-center">
              <MessageCircle className="mx-auto mb-4 h-12 w-12 text-fg/30" />
              <h3 className={cn(typography.h3.size, typography.h3.weight, "mb-2 text-fg/70")}>
                {t("chats.noConversationsYet")}
              </h3>
              <p className={cn(typography.bodySmall.size, "mb-6 text-fg/40")}>
                {t("chats.startChattingPrompt")}
              </p>
            </div>
          ) : filteredSessions.length === 0 ? (
            <div className="py-20 text-center">
              <MessageCircle className="mx-auto mb-4 h-12 w-12 text-fg/30" />
              <h3 className={cn(typography.h3.size, typography.h3.weight, "mb-2 text-fg/70")}>
                {t("chats.noMatchingChats")}
              </h3>
              <p className={cn(typography.bodySmall.size, "mb-6 text-fg/40")}>
                {t("chats.tryDifferentSearchTerm")}
              </p>
              <button
                type="button"
                onClick={() => setQuery("")}
                className={cn(
                  "px-4 py-2 border bg-fg/5 text-fg/80",
                  colors.border.subtle,
                  radius.md,
                  typography.bodySmall.size,
                  interactive.active.scale,
                  interactive.hover.brightness,
                  interactive.transition.fast,
                )}
              >
                {t("common.buttons.clearSearch")}
              </button>
            </div>
          ) : (
            <div className="space-y-6 pb-24">
              {groupedSessions.map((group) => {
                const limit = group.key === "today" || group.key === "older" ? 10 : null;
                const hasPagination = !!limit && group.sessions.length > limit;
                const maxPages = hasPagination ? Math.ceil(group.sessions.length / limit!) : 1;
                const currentPage = Math.min(groupPages[group.key] ?? 1, maxPages);
                const startIndex = hasPagination ? (currentPage - 1) * limit! : 0;
                const visibleSessions = hasPagination
                  ? group.sessions.slice(startIndex, startIndex + limit!)
                  : group.sessions;
                const changePage = (nextPage: number) =>
                  setGroupPages((prev) => ({
                    ...prev,
                    [group.key]: Math.min(Math.max(1, nextPage), maxPages),
                  }));

                return (
                  <section key={group.key}>
                    <div className="mb-3 flex items-center gap-3 px-1">
                      <h2 className={cn(typography.bodySmall.size, "font-medium text-fg/50")}>
                        {group.label}
                      </h2>
                      {hasPagination ? (
                        <div className="ml-auto inline-flex items-center gap-1 rounded-lg border border-fg/8 bg-fg/3 px-1 py-1">
                          <button
                            type="button"
                            onClick={() => changePage(currentPage - 1)}
                            disabled={currentPage <= 1}
                            className={cn(
                              "rounded-md p-1.5 text-fg/58 transition-colors hover:bg-fg/6 hover:text-fg/82",
                              "disabled:pointer-events-none disabled:opacity-30",
                            )}
                            aria-label={t("chats.history.previousGroupPage", { label: group.label })}
                          >
                            <ChevronLeft size={14} />
                          </button>
                          <span
                            className={cn(
                              typography.caption.size,
                              "min-w-12 text-center font-medium text-fg/58",
                            )}
                          >
                            {currentPage} / {maxPages}
                          </span>
                          <button
                            type="button"
                            onClick={() => changePage(currentPage + 1)}
                            disabled={currentPage >= maxPages}
                            className={cn(
                              "rounded-md p-1.5 text-fg/58 transition-colors hover:bg-fg/6 hover:text-fg/82",
                              "disabled:pointer-events-none disabled:opacity-30",
                            )}
                            aria-label={t("chats.history.nextGroupPage", { label: group.label })}
                          >
                            <ChevronRight size={14} />
                          </button>
                        </div>
                      ) : null}
                    </div>
                    <div className="space-y-3">
                      {visibleSessions.map((session) => (
                        <SessionCard
                          key={session.id}
                          session={session}
                          onSelect={() => go(Routes.chatSession(characterId!, session.id))}
                          onDelete={() => setDeleteTarget(session)}
                          onExport={() => setExportTarget(session)}
                          onRename={() => setRenameTarget(session)}
                          isBusy={busyIds.has(session.id)}
                        />
                      ))}
                      {hasPagination ? (
                        <div
                          className={cn(
                            "inline-flex w-full items-center justify-center gap-1 border border-fg/8 bg-fg/3 px-1 py-1",
                            radius.lg,
                          )}
                        >
                          <button
                            type="button"
                            onClick={() => changePage(currentPage - 1)}
                            disabled={currentPage <= 1}
                            className={cn(
                              "rounded-md p-2 text-fg/65 transition-colors hover:bg-fg/6 hover:text-fg/82",
                              "disabled:pointer-events-none disabled:opacity-30",
                            )}
                            aria-label={t("chats.history.previousGroupPage", { label: group.label })}
                          >
                            <ChevronLeft size={16} />
                          </button>
                          <span
                            className={cn(
                              typography.bodySmall.size,
                              "min-w-16 text-center font-medium text-fg/65",
                            )}
                          >
                            {currentPage} / {maxPages}
                          </span>
                          <button
                            type="button"
                            onClick={() => changePage(currentPage + 1)}
                            disabled={currentPage >= maxPages}
                            className={cn(
                              "rounded-md p-2 text-fg/65 transition-colors hover:bg-fg/6 hover:text-fg/82",
                              "disabled:pointer-events-none disabled:opacity-30",
                            )}
                            aria-label={t("chats.history.nextGroupPage", { label: group.label })}
                          >
                            <ChevronRight size={16} />
                          </button>
                        </div>
                      ) : null}
                    </div>
                  </section>
                );
              })}
            </div>
          )}
        </div>
      </main>

      <BottomMenu
        isOpen={renameTarget != null}
        onClose={() => setRenameTarget(null)}
        title={t("common.buttons.rename")}
      >
        <div className="space-y-4 text-fg">
          <div className="rounded-xl border border-fg/10 bg-fg/4 p-3">
            <p className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/90")}>
              {renameTarget?.title || t("chats.untitledChat")}
            </p>
            {renameTarget ? (
              <p className={cn(typography.caption.size, "mt-0.5 text-fg/45")}>
                {formatTimeAgo(renameTarget.updatedAt)}
              </p>
            ) : null}
          </div>

          <input
            type="text"
            value={renameDraft}
            onChange={(e) => setRenameDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== "Enter" || !renameTarget || !renameDraft.trim()) return;
              void (async () => {
                await handleRename(renameTarget.id, renameDraft.trim());
                setRenameTarget(null);
              })();
            }}
            className={cn(
              "w-full border bg-fg/5 px-3 py-2.5 text-fg",
              colors.border.subtle,
              radius.lg,
              typography.bodySmall.size,
              "focus:outline-none focus:ring-2 focus:ring-white/10 focus:border-fg/20",
            )}
            placeholder={t("chats.chatTitlePlaceholder")}
            autoFocus
          />

          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setRenameTarget(null)}
              className={cn(
                "flex-1 border border-fg/10 bg-fg/5 px-4 py-2.5 text-fg/65",
                radius.lg,
                typography.bodySmall.size,
                "font-medium transition-all hover:bg-fg/8 hover:text-fg/80",
                interactive.active.scale,
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              type="button"
              disabled={
                !renameTarget ||
                !renameDraft.trim() ||
                busyIds.has(renameTarget.id) ||
                renameDraft.trim() === renameTarget.title
              }
              onClick={() => {
                if (!renameTarget) return;
                void (async () => {
                  await handleRename(renameTarget.id, renameDraft.trim());
                  setRenameTarget(null);
                })();
              }}
              className={cn(
                "flex-1 border border-accent/30 bg-accent/15 px-4 py-2.5 text-accent/90",
                radius.lg,
                typography.bodySmall.size,
                "font-medium transition-all hover:bg-accent/25",
                interactive.active.scale,
                "disabled:pointer-events-none disabled:opacity-40",
              )}
            >
              {renameTarget && busyIds.has(renameTarget.id)
                ? t("common.buttons.saving")
                : t("common.buttons.save")}
            </button>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={exportTarget != null}
        onClose={() => setExportTarget(null)}
        title={t("chats.exportChatPackage")}
      >
        <div className="rounded-xl border border-fg/10 bg-fg/4 p-3">
          <p className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/90")}>
            {exportTarget?.title || t("chats.untitledChat")}
          </p>
          {exportTarget ? (
            <p className={cn(typography.caption.size, "mt-0.5 text-fg/45")}>
              {formatTimeAgo(exportTarget.updatedAt)}
            </p>
          ) : null}
        </div>

        <MenuDivider />

        <MenuButtonGroup>
          <MenuButton
            icon={User}
            title={exporting ? t("common.buttons.exporting") : t("chats.characterSpecificExport")}
            description={t("chats.characterSpecificExportDesc")}
            color="from-blue-500 to-cyan-600"
            disabled={!exportTarget || exporting}
            onClick={() => {
              void handleExportChatpkg(true);
            }}
          />
          <MenuButton
            icon={Download}
            title={
              exporting ? t("common.buttons.exporting") : t("chats.nonCharacterSpecificExport")
            }
            description={t("chats.nonCharacterSpecificExportDesc")}
            color="from-indigo-500 to-blue-600"
            disabled={!exportTarget || exporting}
            onClick={() => {
              void handleExportChatpkg(false);
            }}
          />
          <MenuButton
            icon={Download}
            title={exporting ? t("common.buttons.exporting") : t("chats.history.sillyTavernFormat")}
            description={t("chats.history.sillyTavernFormatDesc")}
            color="from-emerald-500 to-teal-600"
            disabled={!exportTarget || exporting}
            onClick={() => {
              void handleExportSillyTavern();
            }}
          />
        </MenuButtonGroup>
      </BottomMenu>

      <BottomMenu
        isOpen={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={t("chats.deleteChat")}
        includeExitIcon={false}
      >
        <div className="rounded-xl border border-danger/20 bg-danger/8 p-3">
          <p
            className={cn(
              typography.caption.size,
              "mb-2 uppercase tracking-[0.18em] text-danger/70",
            )}
          >
            {t("chats.deleteChat")}
          </p>
          <p className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/90")}>
            {deleteTarget?.title || t("chats.untitledChat")}
          </p>
          {deleteTarget ? (
            <p className={cn(typography.caption.size, "mt-0.5 text-fg/45")}>
              {formatTimeAgo(deleteTarget.updatedAt)}
            </p>
          ) : null}
          {deleteTarget?.lastMessage ? (
            <p className={cn(typography.bodySmall.size, "mt-2 line-clamp-2 text-fg/60")}>
              {deleteTarget.lastMessage}
            </p>
          ) : null}
          <p className={cn(typography.bodySmall.size, "mt-3 text-fg/55")}>
            {t("chats.deleteConfirmDesc")}
          </p>
        </div>

        <div className="flex gap-2 pt-4">
          <button
            type="button"
            onClick={() => setDeleteTarget(null)}
            disabled={!!deleteTarget && busyIds.has(deleteTarget.id)}
            className={cn(
              "flex-1 border border-fg/10 bg-fg/5 px-4 py-2.5 text-fg/65",
              radius.lg,
              typography.bodySmall.size,
              "font-medium transition-all hover:bg-fg/8 hover:text-fg/80",
              interactive.active.scale,
              "disabled:pointer-events-none disabled:opacity-40",
            )}
          >
            {t("common.buttons.cancel")}
          </button>
          <button
            type="button"
            disabled={!deleteTarget || busyIds.has(deleteTarget.id)}
            onClick={() => {
              if (!deleteTarget) return;
              void (async () => {
                await handleDelete(deleteTarget.id);
                setDeleteTarget(null);
              })();
            }}
            className={cn(
              "flex-1 border border-danger/30 bg-danger/15 px-4 py-2.5 text-danger",
              radius.lg,
              typography.bodySmall.size,
              "font-medium transition-all hover:bg-danger/25",
              interactive.active.scale,
              "disabled:pointer-events-none disabled:opacity-40",
            )}
          >
            {deleteTarget && busyIds.has(deleteTarget.id)
              ? t("common.buttons.deleting")
              : t("chats.deleteChat")}
          </button>
        </div>
      </BottomMenu>
    </div>
  );
}

function SessionCard({
  session,
  onSelect,
  onDelete,
  onExport,
  onRename,
  isBusy,
}: {
  session: SessionPreview;
  onSelect: () => void;
  onDelete: () => void;
  onExport: () => void;
  onRename: () => void;
  isBusy: boolean;
}) {
  const { t } = useI18n();
  const formatTimeAgo = useFormatTimeAgo();

  return (
    <div
      className={cn(
        "group overflow-hidden border transition-colors",
        radius.lg,
        "border-fg/8 bg-fg/3 hover:border-fg/12 hover:bg-fg/4",
        session.archived && "border-amber-400/20 bg-amber-400/5",
      )}
    >
      <button
        onClick={onSelect}
        disabled={isBusy}
        className="w-full px-4 py-3 text-left disabled:opacity-50 transition-colors"
      >
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <div className="mb-1 flex items-center gap-2">
              <h3 className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/92")}>
                {session.title}
              </h3>
              {session.archived ? (
                <span className="inline-flex shrink-0 items-center rounded-md border border-amber-400/25 bg-amber-400/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-200/80">
                  {t("chats.history.archivedBadge")}
                </span>
              ) : null}
            </div>
            {session.lastMessage ? (
              <p
                className={cn(typography.bodySmall.size, "line-clamp-2 leading-relaxed text-fg/58")}
              >
                {session.lastMessage}
              </p>
            ) : (
              <p className={cn(typography.bodySmall.size, "text-fg/32")}>
                {t("chats.untitledChat")}
              </p>
            )}
          </div>
          <span className={cn(typography.caption.size, "shrink-0 pt-0.5 text-fg/35")}>
            {formatTimeAgo(session.updatedAt)}
          </span>
        </div>
      </button>

      <div className="flex items-center justify-between gap-3 border-t border-fg/8 px-3 py-2">
        <span className={cn(typography.caption.size, "text-fg/38")}>
          {t("chats.history.messagesCount", { count: session.messageCount.toLocaleString() })}
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={(e) => {
              e.stopPropagation();
              onRename();
            }}
            disabled={isBusy}
            title={t("common.buttons.rename")}
            className={cn(
              "rounded-lg p-2 text-fg/45 transition-colors hover:bg-fg/6 hover:text-fg/75",
              isBusy && "opacity-50",
            )}
          >
            <Edit3 size={14} />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              onExport();
            }}
            disabled={isBusy}
            title={t("common.buttons.export")}
            className={cn(
              "rounded-lg p-2 text-fg/45 transition-colors hover:bg-fg/6 hover:text-fg/75",
              isBusy && "opacity-50",
            )}
          >
            <Download size={14} />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
            disabled={isBusy}
            title={t("common.buttons.delete")}
            className={cn(
              "rounded-lg p-2 text-fg/35 transition-colors hover:bg-danger/10 hover:text-danger",
              isBusy && "opacity-50",
            )}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}

function useFormatTimeAgo() {
  const { t } = useI18n();
  return (timestamp: number): string => {
    const now = new Date();
    const date = new Date(timestamp);
    const diffMs = now.getTime() - timestamp;
    const diffMinutes = Math.floor(diffMs / (1000 * 60));

    if (diffMinutes < 1) return t("common.time.justNow");
    if (diffMinutes < 60) return t("common.time.minutesAgo", { minutes: diffMinutes });

    const diffHours = Math.floor(diffMinutes / 60);
    const isToday =
      date.getDate() === now.getDate() &&
      date.getMonth() === now.getMonth() &&
      date.getFullYear() === now.getFullYear();

    if (isToday) return t("common.time.hoursAgo", { hours: diffHours });

    const yesterday = new Date(now);
    yesterday.setDate(now.getDate() - 1);
    const isYesterday =
      date.getDate() === yesterday.getDate() &&
      date.getMonth() === yesterday.getMonth() &&
      date.getFullYear() === yesterday.getFullYear();

    if (isYesterday) return t("common.time.yesterday");

    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return t("common.time.daysAgo", { days: diffDays });

    return date.toLocaleDateString();
  };
}

export default ChatHistoryPage;
